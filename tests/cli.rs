//! End-to-end CLI tests for `vet`.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn vet() -> Command {
    cargo_bin_cmd!("vet")
}

#[test]
fn exit_zero_pass() {
    vet()
        .args(["exit", "0", "--", "true"])
        .assert()
        .success()
        .stdout(predicate::str::contains("exit 0"));
}

#[test]
fn exit_zero_fail() {
    vet()
        .args(["exit", "0", "--", "false"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("exit 1"));
}

#[test]
fn exit_nonzero_pass() {
    vet()
        .args(["exit", "nonzero", "--", "false"])
        .assert()
        .success();
}

#[test]
fn exit_nonzero_fail() {
    vet()
        .args(["exit", "nonzero", "--", "true"])
        .assert()
        .code(1);
}

#[test]
fn exit_custom_code() {
    vet()
        .args(["exit", "2", "--", "sh", "-c", "exit 2"])
        .assert()
        .success();
}

#[test]
fn stdout_contains() {
    vet()
        .args(["stdout", "contains", "hello", "--", "echo", "hello world"])
        .assert()
        .success();
}

#[test]
fn stdout_not_contains() {
    vet()
        .args([
            "stdout",
            "!contains",
            "DEPRECATED",
            "--",
            "echo",
            "all good",
        ])
        .assert()
        .success();
}

#[test]
fn stdout_equals() {
    vet()
        .args(["stdout", "equals", "ping", "--", "printf", "ping"])
        .assert()
        .success();
}

#[test]
fn stdout_matches() {
    vet()
        .args(["stdout", "matches", r"[0-9]+", "--", "echo", "v42"])
        .assert()
        .success();
}

#[test]
fn invalid_regex_exit_2() {
    vet()
        .args(["stdout", "matches", "(", "--", "true"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("regex"));
}

#[test]
fn stderr_contains() {
    vet()
        .args([
            "stderr",
            "contains",
            "boom",
            "--",
            "sh",
            "-c",
            "echo boom >&2",
        ])
        .assert()
        .success();
}

#[test]
fn json_equals_status() {
    vet()
        .args([
            "json",
            ".status",
            "==",
            "healthy",
            "--",
            "echo",
            r#"{"status":"healthy"}"#,
        ])
        .assert()
        .success();
}

#[test]
fn json_single_token_expression() {
    vet()
        .args([
            "json",
            r#".status == "healthy""#,
            "--",
            "echo",
            r#"{"status":"healthy"}"#,
        ])
        .assert()
        .success();
}

#[test]
fn json_truthy() {
    vet()
        .args(["json", ".ok", "--", "echo", r#"{"ok":true}"#])
        .assert()
        .success();
}

#[test]
fn json_missing_path_fails() {
    vet()
        .args(["json", ".status", "--", "echo", r#"{"other":1}"#])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("missing"));
}

#[test]
fn json_invalid_body_fails_claim() {
    vet()
        .args(["json", ".x", "--", "echo", "not-json"])
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
        .args(["files", "exist", "Cargo.toml", "--", "true"])
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
        .args(["duration", "lt", "30s", "--", "true"])
        .assert()
        .success();
}

#[test]
fn duration_lt_fail() {
    vet()
        .args(["duration", "lt", "1ms", "--", "sleep", "0.05"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("took"));
}

#[test]
fn jsonl_format() {
    vet()
        .args(["--format", "jsonl", "exit", "0", "--", "true"])
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
        "exit 0 -- true\nexit 0 -- false\n# comment\nenv set PATH\n",
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
        .write_stdin("exit 0 -- true\nexit nonzero -- false\n")
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

/// Integration: script that prints JSON for json claim.
#[test]
fn script_json_pipeline() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("health.sh");
    fs::write(
        &script,
        "#!/bin/sh\necho '{\"status\":\"healthy\",\"ok\":true}'\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();

    vet()
        .args([
            "json",
            ".status",
            "==",
            "healthy",
            "--",
            script.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn timeout_kills_command_exit_2() {
    vet()
        .args(["--timeout", "200ms", "exit", "0", "--", "sleep", "5"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("timed out"));
}

#[test]
fn color_never_has_no_ansi() {
    vet()
        .args(["--color", "never", "exit", "0", "--", "true"])
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
    fs::write(&claims, "exit 0 -- true  # trailing comment\ngit clean\n").unwrap();
    // git clean may pass or fail depending on workspace; only require parse success for first claim path.
    // Use only exit claim for determinism.
    fs::write(&claims, "exit 0 -- true  # trailing comment\n").unwrap();
    vet()
        .args(["-f", claims.to_str().unwrap()])
        .assert()
        .success();
}
