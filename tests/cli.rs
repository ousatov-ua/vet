//! End-to-end CLI tests for `vet`.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn vet() -> Command {
    cargo_bin_cmd!("vet")
}

/// Exit 0 on this platform.
fn ok_cmd() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["cmd", "/C", "exit", "0"]
    }
    #[cfg(not(windows))]
    {
        &["true"]
    }
}

/// Exit 1 on this platform.
fn fail_cmd() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["cmd", "/C", "exit", "1"]
    }
    #[cfg(not(windows))]
    {
        &["false"]
    }
}

/// Shell form of [`ok_cmd`] for batch claim lines.
fn ok_cmd_line() -> &'static str {
    #[cfg(windows)]
    {
        "cmd /C exit 0"
    }
    #[cfg(not(windows))]
    {
        "true"
    }
}

/// Shell form of [`fail_cmd`] for batch claim lines.
fn fail_cmd_line() -> &'static str {
    #[cfg(windows)]
    {
        "cmd /C exit 1"
    }
    #[cfg(not(windows))]
    {
        "false"
    }
}

/// Print `text` to stdout (no extra newline where possible).
fn emit_args(text: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        // Avoid cmd `echo` quote/space quirks on Windows.
        vec![
            "powershell".into(),
            "-NoProfile".into(),
            "-Command".into(),
            format!("[Console]::Out.Write('{text}')"),
        ]
    }
    #[cfg(not(windows))]
    {
        vec!["printf".into(), "%s".into(), text.into()]
    }
}

/// Sleep for roughly `ms` milliseconds.
fn sleep_args(ms: u64) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "powershell".into(),
            "-NoProfile".into(),
            "-Command".into(),
            format!("Start-Sleep -Milliseconds {ms}"),
        ]
    }
    #[cfg(not(windows))]
    {
        if ms >= 1000 && ms % 1000 == 0 {
            vec!["sleep".into(), format!("{}", ms / 1000)]
        } else {
            vec!["sleep".into(), format!("{:.3}", ms as f64 / 1000.0)]
        }
    }
}

/// Print a file's contents to stdout.
fn cat_args(path: &Path) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "cmd".into(),
            "/C".into(),
            "type".into(),
            path.display().to_string(),
        ]
    }
    #[cfg(not(windows))]
    {
        vec!["cat".into(), path.display().to_string()]
    }
}

/// Exit with the given status code.
fn exit_code_cmd(code: i32) -> Vec<String> {
    #[cfg(windows)]
    {
        vec!["cmd".into(), "/C".into(), "exit".into(), code.to_string()]
    }
    #[cfg(not(windows))]
    {
        vec!["sh".into(), "-c".into(), format!("exit {code}")]
    }
}

/// Write `text` to stderr.
fn emit_stderr_args(text: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            "powershell".into(),
            "-NoProfile".into(),
            "-Command".into(),
            format!("[Console]::Error.Write('{text}')"),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![
            "sh".into(),
            "-c".into(),
            format!("printf '%s' '{text}' >&2"),
        ]
    }
}


#[test]
fn exit_zero_pass() {
    vet()
        .args(["exit", "0", "--"]).args(ok_cmd())
        .assert()
        .success()
        .stdout(predicate::str::contains("exit 0"));
}

#[test]
fn exit_zero_fail() {
    vet()
        .args(["exit", "0", "--"]).args(fail_cmd())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("exit 1"));
}

#[test]
fn exit_nonzero_pass() {
    vet()
        .args(["exit", "nonzero", "--"]).args(fail_cmd())
        .assert()
        .success();
}

#[test]
fn exit_nonzero_fail() {
    vet()
        .args(["exit", "nonzero", "--"]).args(ok_cmd())
        .assert()
        .code(1);
}

#[test]
fn exit_custom_code() {
    vet()
        .args(["exit", "2", "--"]).args(exit_code_cmd(2))
        .assert()
        .success();
}

#[test]
fn stdout_contains() {
    vet()
        .args(["stdout", "contains", "hello", "--"]).args(emit_args("hello world"))
        .assert()
        .success();
}

#[test]
fn stdout_not_contains() {
    vet()
        .args(["stdout", "!contains", "DEPRECATED", "--"])
        .args(emit_args("all good"))
        .assert()
        .success();
}

#[test]
fn stdout_equals() {
    vet()
        .args(["stdout", "equals", "ping", "--"]).args(emit_args("ping"))
        .assert()
        .success();
}

#[test]
fn stdout_matches() {
    vet()
        .args(["stdout", "matches", r"[0-9]+", "--"]).args(emit_args("v42"))
        .assert()
        .success();
}

#[test]
fn invalid_regex_exit_2() {
    vet()
        .args(["stdout", "matches", "(", "--"]).args(ok_cmd())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("regex"));
}

#[test]
fn stderr_contains() {
    vet()
        .args(["stderr", "contains", "boom", "--"])
        .args(emit_stderr_args("boom"))
        .assert()
        .success();
}

#[test]
fn json_equals_status() {
    vet()
        .args(["json", ".status", "==", "healthy", "--"])
        .args(emit_args(r#"{"status":"healthy"}"#))
        .assert()
        .success();
}

#[test]
fn json_single_token_expression() {
    vet()
        .args(["json", r#".status == "healthy""#, "--"])
        .args(emit_args(r#"{"status":"healthy"}"#))
        .assert()
        .success();
}

#[test]
fn json_truthy() {
    vet()
        .args(["json", ".ok", "--"]).args(emit_args(r#"{"ok":true}"#))
        .assert()
        .success();
}

#[test]
fn json_missing_path_fails() {
    vet()
        .args(["json", ".status", "--"]).args(emit_args(r#"{"other":1}"#))
        .assert()
        .code(1)
        .stdout(predicate::str::contains("missing"));
}

#[test]
fn json_invalid_body_fails_claim() {
    vet()
        .args(["json", ".x", "--"]).args(emit_args("not-json"))
        .assert()
        .code(1)
        .stdout(predicate::str::contains("invalid json"));
}

#[test]
fn files_exist() {
    vet()
        .args(["files", "exist", "Cargo.toml", "src/main.rs"])
        .assert()
        .success();
}

#[test]
fn files_missing_fails() {
    vet()
        .args(["files", "exist", "no-such-file-xyz"])
        .assert()
        .code(1);
}

#[test]
fn files_not_exist() {
    vet()
        .args(["files", "!exist", "no-such-file-xyz"])
        .assert()
        .success();
}

#[test]
fn files_rejects_command() {
    vet()
        .args(["files", "exist", "Cargo.toml", "--"]).args(ok_cmd())
        .assert()
        .code(2);
}

#[test]
fn env_set_path() {
    vet().args(["env", "set", "PATH"]).assert().success();
}

#[test]
fn env_not_set() {
    vet()
        .env_remove("VET_TEST_UNSET_VAR")
        .args(["env", "!set", "VET_TEST_UNSET_VAR"])
        .assert()
        .success();
}

#[test]
fn env_evidence_hides_value() {
    vet()
        .env("VET_SECRET", "super-secret-value")
        .args(["env", "set", "VET_SECRET"])
        .assert()
        .success()
        .stdout(predicate::str::contains("super-secret-value").not());
}

#[test]
fn git_clean_in_temp_repo() {
    let dir = TempDir::new().unwrap();
    StdCommand::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "t@example.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "t"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    // empty repo may be dirty if no commits; commit empty tree optional
    // porcelain empty after init with no files → clean
    vet()
        .current_dir(dir.path())
        .args(["git", "clean"])
        .assert()
        .success();
}

#[test]
fn git_dirty_detects_file() {
    let dir = TempDir::new().unwrap();
    StdCommand::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    fs::write(dir.path().join("x.txt"), "x").unwrap();
    vet()
        .current_dir(dir.path())
        .args(["git", "dirty"])
        .assert()
        .success();
}

#[test]
fn git_not_repo_claim_fail() {
    let dir = TempDir::new().unwrap();
    vet()
        .current_dir(dir.path())
        .args(["git", "clean"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("not a git repository"));
}

#[test]
fn duration_lt_pass() {
    vet()
        .args(["duration", "lt", "30s", "--"]).args(ok_cmd())
        .assert()
        .success();
}

#[test]
fn duration_lt_fail() {
    vet()
        .args(["duration", "lt", "1ms", "--"]).args(sleep_args(50))
        .assert()
        .code(1)
        .stdout(predicate::str::contains("took"));
}

#[test]
fn jsonl_format() {
    vet()
        .args(["--format", "jsonl", "exit", "0", "--"]).args(ok_cmd())
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""ok":true"#))
        .stdout(predicate::str::contains(r#""claim":"exit 0""#));
}

#[test]
fn batch_file_mixed() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("claims.txt");
    fs::write(
        &path,
        format!(
            "exit 0 -- {}\nexit 0 -- {}\n# comment\nenv set PATH\n",
            ok_cmd_line(),
            fail_cmd_line()
        ),
    )
    .unwrap();
    vet()
        .args(["-f", path.to_str().unwrap(), "--format", "jsonl"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(r#""ok":true"#))
        .stdout(predicate::str::contains(r#""ok":false"#));
}

#[test]
fn batch_stdin() {
    vet()
        .args(["-f", "-", "--format", "jsonl"])
        .write_stdin(format!(
            "exit 0 -- {}\nexit nonzero -- {}\n",
            ok_cmd_line(),
            fail_cmd_line()
        ))
        .assert()
        .success();
}

#[test]
fn usage_error_no_args() {
    vet()
        .args([] as [&str; 0])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("usage"));
}

#[test]
fn command_required() {
    vet()
        .args(["exit", "0"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("requires a command"));
}

#[test]
fn spawn_failure_exit_2() {
    vet()
        .args(["exit", "0", "--", "definitely-not-a-binary-xyz"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("spawn"));
}

#[test]
fn help_exits_0() {
    vet().arg("--help").assert().success();
}

#[test]
fn unknown_claim_kind() {
    vet()
        .args(["foobar", "x"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown claim"));
}

/// Integration: print JSON via a platform-native file dump for json claim.
#[test]
fn script_json_pipeline() {
    let dir = TempDir::new().unwrap();
    let payload = dir.path().join("health.json");
    fs::write(&payload, r#"{"status":"healthy","ok":true}"#).unwrap();

    vet()
        .args(["json", ".status", "==", "healthy", "--"])
        .args(cat_args(&payload))
        .assert()
        .success();
}

#[test]
fn timeout_kills_command_exit_2() {
    vet()
        .args(["--timeout", "200ms", "exit", "0", "--"]).args(sleep_args(5000))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("timed out"));
}

#[test]
fn color_never_has_no_ansi() {
    vet()
        .args(["--color", "never", "exit", "0", "--"]).args(ok_cmd())
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::function(|s: &str| !s.contains('\u{1b}')));
}

#[test]
fn files_after_double_dash_friendly_error() {
    vet()
        .args(["files", "exist", "--", "README.md"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("paths as arguments"));
}

#[test]
fn batch_mid_line_comment() {
    let dir = TempDir::new().unwrap();
    let claims = dir.path().join("c.txt");
    // Use only exit claim for determinism.
    fs::write(
        &claims,
        format!("exit 0 -- {}  # trailing comment\n", ok_cmd_line()),
    )
    .unwrap();
    vet()
        .args(["-f", claims.to_str().unwrap()])
        .assert()
        .success();
}
