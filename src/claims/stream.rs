//! `stdout` / `stderr` stream claims.

use super::Verdict;
use crate::error::{Result, VclaimError};
use crate::run::RunResult;
use crate::util::truncate_chars;
use regex::Regex;

/// Which stream to inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Stdout,
    Stderr,
}

impl StreamKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StreamKind::Stdout => "stdout",
            StreamKind::Stderr => "stderr",
        }
    }
}

/// Comparison operation on a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamOp {
    Contains,
    NotContains,
    Equals,
    Matches,
}

impl StreamOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamOp::Contains => "contains",
            StreamOp::NotContains => "!contains",
            StreamOp::Equals => "equals",
            StreamOp::Matches => "matches",
        }
    }
}

/// Claim: `stdout contains NEEDLE`, `stderr !contains X`, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamClaim {
    pub stream: StreamKind,
    pub op: StreamOp,
    pub needle: String,
}

impl StreamClaim {
    pub fn display(&self) -> String {
        format!(
            "{} {} {}",
            self.stream.as_str(),
            self.op.as_str(),
            shell_quote(&self.needle)
        )
    }

    pub fn evaluate(&self, run: &RunResult) -> Verdict {
        let claim = self.display();
        let body = match self.stream {
            StreamKind::Stdout => &run.stdout,
            StreamKind::Stderr => &run.stderr,
        };

        match self.op {
            StreamOp::Contains => {
                if body.contains(&self.needle) {
                    Verdict::pass(claim, "matched").with_run(run)
                } else {
                    Verdict::fail(claim, snippet_evidence(body, "not found")).with_run(run)
                }
            }
            StreamOp::NotContains => {
                if body.contains(&self.needle) {
                    Verdict::fail(claim, snippet_evidence(body, "found")).with_run(run)
                } else {
                    Verdict::pass(claim, "absent").with_run(run)
                }
            }
            StreamOp::Equals => {
                let normalized = strip_one_trailing_newline(body);
                let expected = strip_one_trailing_newline(&self.needle);
                if normalized == expected {
                    Verdict::pass(claim, "equals").with_run(run)
                } else {
                    Verdict::fail(claim, snippet_evidence(body, "not equal")).with_run(run)
                }
            }
            StreamOp::Matches => match evaluate_matches(body, &self.needle) {
                Ok(true) => Verdict::pass(claim, "matched").with_run(run),
                Ok(false) => Verdict::fail(claim, snippet_evidence(body, "no match")).with_run(run),
                Err(msg) => Verdict::fail(claim, msg).with_run(run),
            },
        }
    }

    /// Compile regex early so invalid patterns are usage/parse errors at parse time.
    pub fn validate_regex(&self) -> Result<()> {
        if matches!(self.op, StreamOp::Matches) {
            Regex::new(&self.needle)
                .map_err(|e| VclaimError::Parse(format!("invalid regex `{}`: {e}", self.needle)))?;
        }
        Ok(())
    }
}

fn evaluate_matches(body: &str, pattern: &str) -> std::result::Result<bool, String> {
    let re = Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
    Ok(re.is_match(body))
}

fn strip_one_trailing_newline(s: &str) -> &str {
    s.strip_suffix('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .unwrap_or(s)
}

fn snippet_evidence(body: &str, label: &str) -> String {
    let snippet = body.lines().next().unwrap_or(body);
    let snippet = truncate_chars(snippet, 80);
    if snippet.is_empty() {
        format!("{label} (empty stream)")
    } else {
        format!("{label}: {snippet}")
    }
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() || s.chars().any(|c| c.is_whitespace() || "\"'\\".contains(c)) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_out(stdout: &str) -> RunResult {
        RunResult {
            exit_code: Some(0),
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
            ms: 1,
            command_display: "cmd".into(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn snippet_multibyte_no_panic() {
        let body: String = "é".repeat(100);
        let evidence = snippet_evidence(&body, "not found");
        assert!(evidence.starts_with("not found:"));
        assert!(evidence.contains('…'));
    }

    #[test]
    fn contains_hit() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::Contains,
            needle: "ok".into(),
        };
        assert!(c.evaluate(&run_out("all ok here")).ok);
    }

    #[test]
    fn contains_miss() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::Contains,
            needle: "fail".into(),
        };
        assert!(!c.evaluate(&run_out("all ok here")).ok);
    }

    #[test]
    fn not_contains() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::NotContains,
            needle: "DEPRECATED".into(),
        };
        assert!(c.evaluate(&run_out("ok")).ok);
        assert!(!c.evaluate(&run_out("DEPRECATED flag")).ok);
    }

    #[test]
    fn equals_strips_one_newline() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::Equals,
            needle: "hello".into(),
        };
        assert!(c.evaluate(&run_out("hello\n")).ok);
    }

    #[test]
    fn matches_regex() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::Matches,
            needle: r"\d{3}".into(),
        };
        assert!(c.evaluate(&run_out("code 404")).ok);
        assert!(!c.evaluate(&run_out("no digits")).ok);
    }

    #[test]
    fn invalid_regex_validate() {
        let c = StreamClaim {
            stream: StreamKind::Stdout,
            op: StreamOp::Matches,
            needle: "(".into(),
        };
        assert!(c.validate_regex().is_err());
    }
}
