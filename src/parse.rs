//! Claim and batch parsing.

use crate::claims::{
    parse_duration_token, Claim, CommandPolicy, DurationClaim, DurationOp, EnvClaim, ExitClaim,
    ExitExpect, FilesClaim, GitClaim, JsonClaim, JsonOp, StreamClaim, StreamKind, StreamOp,
};
use crate::cli::Cli;
use crate::error::{Result, VclaimError};
use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;

/// One claim job: claim AST + optional command argv.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimJob {
    pub claim: Claim,
    /// Command argv when the claim requires a process (after `--`).
    pub command: Option<Vec<String>>,
}

/// Collect claim jobs from CLI (single claim, `-f`, or stdin batch).
pub fn collect_jobs(cli: &Cli) -> Result<Vec<ClaimJob>> {
    if let Some(path) = &cli.file {
        let text = if path.as_os_str() == "-" {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            buf
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| VclaimError::Io(format!("cannot read {}: {e}", path.display())))?
        };
        return parse_batch(&text);
    }

    if cli.rest.is_empty() {
        // No args: if stdin is not a tty, treat as batch; else usage.
        if !io::IsTerminal::is_terminal(&io::stdin()) {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            if !buf.trim().is_empty() {
                return parse_batch(&buf);
            }
        }
        return Err(VclaimError::Usage(
            "provide a claim (e.g. `vclaim exit 0 -- true`) or `-f claims.txt`".into(),
        ));
    }

    // Rest may still contain `--` as a token.
    let (claim_tokens, command) = split_command(cli.rest.clone());
    Ok(vec![parse_job(&claim_tokens, command)?])
}

/// Parse a batch file / stdin body (one claim per line).
///
/// Blank lines and `#` comments (full-line or mid-line outside quotes) are skipped.
pub fn parse_batch(text: &str) -> Result<Vec<ClaimJob>> {
    let mut jobs = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let tokens = tokenize(line).map_err(|e| VclaimError::Parse(format!("line {}: {e}", i + 1)))?;
        let (claim_tokens, command) = split_command(tokens);
        let job = parse_job(&claim_tokens, command).map_err(|e| match e {
            VclaimError::Parse(m) => VclaimError::Parse(format!("line {}: {m}", i + 1)),
            VclaimError::Usage(m) => VclaimError::Usage(format!("line {}: {m}", i + 1)),
            other => other,
        })?;
        jobs.push(job);
    }
    if jobs.is_empty() {
        return Err(VclaimError::Usage("no claims found in input".into()));
    }
    Ok(jobs)
}

/// Parse a single claim line (no file context). Useful for tests.
pub fn parse_line(line: &str) -> Result<ClaimJob> {
    let line = strip_comment(line).trim();
    if line.is_empty() {
        return Err(VclaimError::Usage("empty claim".into()));
    }
    let tokens = tokenize(line)?;
    let (claim_tokens, command) = split_command(tokens);
    parse_job(&claim_tokens, command)
}

fn parse_job(claim_tokens: &[String], command: Option<Vec<String>>) -> Result<ClaimJob> {
    if claim_tokens.is_empty() {
        return Err(VclaimError::Usage("missing claim".into()));
    }

    // Common footgun: `files exist -- path` moves paths after `--` before claim parse.
    if command.is_some() {
        match claim_tokens[0].as_str() {
            "files" | "env" | "git" => {
                return Err(workspace_command_error_kind(claim_tokens[0].as_str()));
            }
            _ => {}
        }
    }

    let claim = parse_claim(claim_tokens)?;

    match claim.command_policy() {
        CommandPolicy::Required => {
            if command.is_none() {
                return Err(VclaimError::Usage(format!(
                    "claim `{}` requires a command after `--`",
                    claim.display()
                )));
            }
        }
        CommandPolicy::Forbidden => {
            // Defensive: should already be caught above.
            if command.is_some() {
                return Err(workspace_command_error(&claim));
            }
        }
    }

    Ok(ClaimJob { claim, command })
}

/// Friendlier errors when workspace claims are written with a command after `--`.
fn workspace_command_error(claim: &Claim) -> VclaimError {
    workspace_command_error_kind(match claim {
        Claim::Files(_) => "files",
        Claim::Env(_) => "env",
        Claim::Git(_) => "git",
        _ => "claim",
    })
}

fn workspace_command_error_kind(kind: &str) -> VclaimError {
    let hint = match kind {
        "files" => {
            "files claims take paths as arguments, not after `--` (try: files exist <path>…)"
        }
        "env" => {
            "env claims take a variable name as an argument, not after `--` (try: env set NAME)"
        }
        "git" => "git claims do not take a command (try: git clean)",
        _ => "this claim does not accept a command",
    };
    VclaimError::Usage(format!("claim `{kind}`: {hint}"))
}

fn parse_claim(tokens: &[String]) -> Result<Claim> {
    let kind = tokens[0].as_str();
    let args: Vec<&str> = tokens[1..].iter().map(String::as_str).collect();

    match kind {
        "exit" => parse_exit(&args),
        "stdout" => parse_stream(StreamKind::Stdout, &args),
        "stderr" => parse_stream(StreamKind::Stderr, &args),
        "json" => parse_json(&args),
        "files" => parse_files(&args),
        "env" => parse_env(&args),
        "git" => parse_git(&args),
        "duration" => parse_duration(&args),
        other => Err(VclaimError::Parse(format!(
            "unknown claim kind `{other}` (expected exit|stdout|stderr|json|files|env|git|duration)"
        ))),
    }
}

fn parse_exit(args: &[&str]) -> Result<Claim> {
    let [raw] = args else {
        return Err(VclaimError::Parse(
            "exit claim expects one argument: `0`, `nonzero`, or an integer".into(),
        ));
    };
    let expect = if *raw == "nonzero" {
        ExitExpect::Nonzero
    } else {
        let code: i32 = raw
            .parse()
            .map_err(|_| VclaimError::Parse(format!("invalid exit code `{raw}`")))?;
        ExitExpect::Code(code)
    };
    Ok(Claim::Exit(ExitClaim { expect }))
}

fn parse_stream(kind: StreamKind, args: &[&str]) -> Result<Claim> {
    let [op_s, needle] = args else {
        return Err(VclaimError::Parse(format!(
            "{} claim expects: contains|!contains|equals|matches NEEDLE",
            kind.as_str()
        )));
    };
    let op = match *op_s {
        "contains" => StreamOp::Contains,
        "!contains" => StreamOp::NotContains,
        "equals" => StreamOp::Equals,
        "matches" => StreamOp::Matches,
        other => {
            return Err(VclaimError::Parse(format!(
                "unknown {} op `{other}` (contains|!contains|equals|matches)",
                kind.as_str()
            )));
        }
    };
    let claim = StreamClaim {
        stream: kind,
        op,
        needle: (*needle).to_string(),
    };
    claim.validate_regex()?;
    Ok(Claim::Stream(claim))
}

fn parse_json(args: &[&str]) -> Result<Claim> {
    if args.is_empty() {
        return Err(VclaimError::Parse(
            "json claim expects: PATH [exists|== VALUE]".into(),
        ));
    }

    // Shell form: json '.status == "healthy"' → one expression token.
    if args.len() == 1 && json_expr_needs_resplit(args[0]) {
        let inner = tokenize(args[0])?;
        let inner_refs: Vec<&str> = inner.iter().map(String::as_str).collect();
        return parse_json(&inner_refs);
    }

    let path = normalize_path_token(args[0]);
    let op = match args.len() {
        1 => JsonOp::Truthy,
        2 if args[1] == "exists" => JsonOp::Exists,
        3 if args[1] == "==" => JsonOp::Equals(parse_json_value(args[2])?),
        n if n >= 3 && args[1] == "==" => {
            // Allow unquoted multi-token values: join remaining as bare string / JSON.
            let raw = args[2..].join(" ");
            JsonOp::Equals(parse_json_value(&raw)?)
        }
        _ => {
            return Err(VclaimError::Parse(
                "json claim expects: PATH | PATH exists | PATH == VALUE".into(),
            ));
        }
    };
    Ok(Claim::Json(JsonClaim { path, op }))
}

fn json_expr_needs_resplit(token: &str) -> bool {
    token.contains(char::is_whitespace)
        || token.contains("==")
        || token.ends_with(" exists")
        || token.contains(" exists ")
}

fn normalize_path_token(token: &str) -> String {
    token.strip_prefix('.').unwrap_or(token).to_string()
}

/// Prefer JSON literals; bare words become strings.
fn parse_json_value(token: &str) -> Result<Value> {
    if let Ok(v) = serde_json::from_str::<Value>(token) {
        return Ok(v);
    }
    Ok(Value::String(token.to_string()))
}

fn parse_files(args: &[&str]) -> Result<Claim> {
    if args.is_empty() {
        return Err(VclaimError::Parse(
            "files claim expects: exist|!exist PATH…".into(),
        ));
    }
    let should_exist = match args[0] {
        "exist" | "exists" => true,
        "!exist" | "!exists" => false,
        other => {
            return Err(VclaimError::Parse(format!(
                "unknown files op `{other}` (exist|!exist)"
            )));
        }
    };
    let paths: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err(VclaimError::Parse(
            "files claim requires at least one path".into(),
        ));
    }
    Ok(Claim::Files(FilesClaim {
        should_exist,
        paths,
    }))
}

fn parse_env(args: &[&str]) -> Result<Claim> {
    if args.len() < 2 {
        return Err(VclaimError::Parse("env claim expects: set|!set NAME…".into()));
    }
    let should_be_set = match args[0] {
        "set" => true,
        "!set" => false,
        other => {
            return Err(VclaimError::Parse(format!(
                "unknown env op `{other}` (set|!set)"
            )));
        }
    };
    let names: Vec<String> = args[1..].iter().map(|s| (*s).to_string()).collect();
    if names.iter().any(|n| n.is_empty()) {
        return Err(VclaimError::Parse(
            "env variable name must be non-empty".into(),
        ));
    }
    Ok(Claim::Env(EnvClaim {
        should_be_set,
        names,
    }))
}

fn parse_git(args: &[&str]) -> Result<Claim> {
    let [mode] = args else {
        return Err(VclaimError::Parse("git claim expects: clean|dirty".into()));
    };
    let claim = match *mode {
        "clean" => GitClaim::Clean,
        "dirty" => GitClaim::Dirty,
        other => {
            return Err(VclaimError::Parse(format!(
                "unknown git mode `{other}` (clean|dirty)"
            )));
        }
    };
    Ok(Claim::Git(claim))
}

fn parse_duration(args: &[&str]) -> Result<Claim> {
    let [op_s, limit_s] = args else {
        return Err(VclaimError::Parse(
            "duration claim expects: lt DURATION (e.g. lt 30s)".into(),
        ));
    };
    if *op_s != "lt" {
        return Err(VclaimError::Parse(format!(
            "unknown duration op `{op_s}` (only `lt` in v0.1)"
        )));
    }
    let limit = parse_duration_token(limit_s).map_err(VclaimError::Parse)?;
    Ok(Claim::Duration(DurationClaim {
        op: DurationOp::Lt,
        limit,
    }))
}

/// Split tokens on the first bare `--` into (claim_tokens, optional command argv).
fn split_command(tokens: Vec<String>) -> (Vec<String>, Option<Vec<String>>) {
    if let Some(pos) = tokens.iter().position(|t| t == "--") {
        let claim = tokens[..pos].to_vec();
        let cmd = tokens[pos + 1..].to_vec();
        if cmd.is_empty() {
            (claim, None)
        } else {
            (claim, Some(cmd))
        }
    } else {
        (tokens, None)
    }
}

/// Shell-ish tokenizer: whitespace split with single/double quotes.
fn tokenize(input: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '\\' if in_double => match chars.next() {
                Some(n) => cur.push(n),
                None => cur.push('\\'),
            },
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }

    if in_single || in_double {
        return Err(VclaimError::Parse("unclosed quote".into()));
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    Ok(tokens)
}

/// Strip `#` comments outside quotes. Mid-line `#` ends the claim.
fn strip_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = line.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if in_double => {
                let _ = chars.next();
            }
            '#' if !in_single && !in_double => return &line[..i],
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exit_command() {
        let job = parse_line("exit 0 -- cargo test -q").unwrap();
        assert!(matches!(
            job.claim,
            Claim::Exit(ExitClaim {
                expect: ExitExpect::Code(0)
            })
        ));
        assert_eq!(
            job.command.as_deref(),
            Some(["cargo".to_string(), "test".to_string(), "-q".to_string()].as_slice())
        );
    }

    #[test]
    fn parse_git_no_command() {
        let job = parse_line("git clean").unwrap();
        assert!(matches!(job.claim, Claim::Git(GitClaim::Clean)));
        assert!(job.command.is_none());
    }

    #[test]
    fn parse_stdout_quoted() {
        let job = parse_line(r#"stdout contains "hello world" -- echo hi"#).unwrap();
        match job.claim {
            Claim::Stream(s) => {
                assert_eq!(s.needle, "hello world");
                assert_eq!(s.op, StreamOp::Contains);
            }
            _ => panic!("expected stream"),
        }
    }

    #[test]
    fn parse_json_equals() {
        let job = parse_line(r#"json .status == "healthy" -- curl x"#).unwrap();
        match job.claim {
            Claim::Json(j) => {
                assert_eq!(j.path, "status");
                assert_eq!(j.op, JsonOp::Equals(serde_json::json!("healthy")));
            }
            _ => panic!("expected json"),
        }
    }

    #[test]
    fn parse_json_bare_string_value() {
        let job = parse_line("json .status == healthy -- curl x").unwrap();
        match job.claim {
            Claim::Json(j) => {
                assert_eq!(j.op, JsonOp::Equals(serde_json::json!("healthy")));
            }
            _ => panic!("expected json"),
        }
    }

    #[test]
    fn parse_json_single_token_expr() {
        let job = parse_line(r#"json '.status == "healthy"' -- curl x"#).unwrap();
        match job.claim {
            Claim::Json(j) => {
                assert_eq!(j.path, "status");
                assert_eq!(j.op, JsonOp::Equals(serde_json::json!("healthy")));
            }
            _ => panic!("expected json"),
        }
    }

    #[test]
    fn missing_command_errors() {
        let err = parse_line("exit 0").unwrap_err();
        assert!(err.to_string().contains("requires a command"));
    }

    #[test]
    fn command_on_git_errors() {
        let err = parse_line("git clean -- true").unwrap_err();
        assert!(err.to_string().contains("do not take a command"));
    }

    #[test]
    fn files_with_command_friendly_error() {
        let err = parse_line("files exist -- README.md").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("paths as arguments") || msg.contains("not after"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn batch_skips_comments_and_blanks() {
        let jobs = parse_batch(
            r#"
# full-line comment
exit 0 -- true
git clean  # mid-line comment
"#,
        )
        .unwrap();
        assert_eq!(jobs.len(), 2);
        assert!(matches!(jobs[1].claim, Claim::Git(GitClaim::Clean)));
    }

    #[test]
    fn mid_line_comment_preserves_hash_in_quotes() {
        let job = parse_line(r#"stdout contains "a#b" -- true"#).unwrap();
        match job.claim {
            Claim::Stream(s) => assert_eq!(s.needle, "a#b"),
            _ => panic!("expected stream"),
        }
    }

    #[test]
    fn tokenize_quotes() {
        let t = tokenize(r#"a "b c" 'd e'"#).unwrap();
        assert_eq!(t, vec!["a", "b c", "d e"]);
    }

    #[test]
    fn invalid_regex_at_parse() {
        let err = parse_line(r#"stdout matches "(" -- true"#).unwrap_err();
        assert!(err.to_string().contains("invalid regex"));
    }

    #[test]
    fn duration_parse() {
        let job = parse_line("duration lt 500ms -- true").unwrap();
        match job.claim {
            Claim::Duration(d) => assert_eq!(d.limit, std::time::Duration::from_millis(500)),
            _ => panic!("expected duration"),
        }
    }

    #[test]
    fn command_policy_required() {
        let job = parse_line("exit 0 -- true").unwrap();
        assert_eq!(job.claim.command_policy(), CommandPolicy::Required);
    }

    #[test]
    fn command_policy_forbidden() {
        let job = parse_line("env set CI").unwrap();
        assert_eq!(job.claim.command_policy(), CommandPolicy::Forbidden);
    }
}
