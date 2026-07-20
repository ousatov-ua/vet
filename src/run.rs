//! Command runner: spawn, capture streams, measure wall-clock duration.

use crate::error::{Result, VetError};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

/// Default per-stream capture cap (1 MiB).
pub const DEFAULT_OUTPUT_CAP: usize = 1_048_576;

/// Options for running a claim command.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Kill the process after this wall-clock limit (`None` = no limit).
    pub timeout: Option<Duration>,
    /// Max bytes kept per stream (stdout / stderr). Excess is discarded.
    pub output_cap: usize,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            timeout: None,
            output_cap: DEFAULT_OUTPUT_CAP,
        }
    }
}

/// Captured result of running a command.
#[derive(Debug, Clone)]
pub struct RunResult {
    /// Process exit code, or `None` if terminated by signal.
    pub exit_code: Option<i32>,
    /// Whether the process exited with status success (code 0).
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// Wall-clock duration in milliseconds.
    pub ms: u64,
    /// Display form of the argv (shell-ish join for evidence).
    pub command_display: String,
    /// True if stdout exceeded the capture cap.
    pub stdout_truncated: bool,
    /// True if stderr exceeded the capture cap.
    pub stderr_truncated: bool,
}

/// Run `argv[0]` with `argv[1..]` as args (no shell). Uses default options.
pub fn run_command(argv: &[String]) -> Result<RunResult> {
    run_command_with(argv, &RunOptions::default())
}

/// Run with explicit timeout / output caps.
pub fn run_command_with(argv: &[String], opts: &RunOptions) -> Result<RunResult> {
    if argv.is_empty() {
        return Err(VetError::Usage("empty command".into()));
    }

    let display = argv.join(" ");
    let mut cmd = Command::new(&argv[0]);
    if argv.len() > 1 {
        cmd.args(&argv[1..]);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|source| VetError::Spawn {
        command: display.clone(),
        source,
    })?;

    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| VetError::Io("missing stdout pipe".into()))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| VetError::Io("missing stderr pipe".into()))?;

    let cap = opts.output_cap;
    let stdout_thr = std::thread::spawn(move || read_capped(stdout_pipe, cap));
    let stderr_thr = std::thread::spawn(move || read_capped(stderr_pipe, cap));

    let status = match opts.timeout {
        Some(limit) => match child
            .wait_timeout(limit)
            .map_err(|e| VetError::Io(format!("wait failed for `{display}`: {e}")))?
        {
            Some(st) => st,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thr.join();
                let _ = stderr_thr.join();
                return Err(VetError::Timeout {
                    command: display,
                    limit,
                });
            }
        },
        None => child
            .wait()
            .map_err(|e| VetError::Io(format!("wait failed for `{display}`: {e}")))?,
    };

    let ms = start.elapsed().as_millis() as u64;

    let (stdout_bytes, stdout_truncated) = stdout_thr
        .join()
        .map_err(|_| VetError::Io("stdout reader panicked".into()))?
        .map_err(|e| VetError::Io(format!("reading stdout of `{display}`: {e}")))?;
    let (stderr_bytes, stderr_truncated) = stderr_thr
        .join()
        .map_err(|_| VetError::Io("stderr reader panicked".into()))?
        .map_err(|e| VetError::Io(format!("reading stderr of `{display}`: {e}")))?;

    let exit_code = status.code();
    Ok(RunResult {
        exit_code,
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        ms,
        command_display: display,
        stdout_truncated,
        stderr_truncated,
    })
}

/// Read up to `cap` bytes; drain the rest and flag truncation.
fn read_capped(mut r: impl Read, cap: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        let n = r.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        if buf.len() >= cap {
            truncated = true;
            let mut drain = [0u8; 8192];
            while r.read(&mut drain)? > 0 {}
            break;
        }
        let space = cap - buf.len();
        let take = n.min(space);
        buf.extend_from_slice(&chunk[..take]);
        if take < n {
            truncated = true;
            let mut drain = [0u8; 8192];
            while r.read(&mut drain)? > 0 {}
            break;
        }
    }
    Ok((buf, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_true() {
        let r = run_command(&["true".into()]).unwrap();
        assert!(r.success);
        assert_eq!(r.exit_code, Some(0));
    }

    #[test]
    fn runs_false() {
        let r = run_command(&["false".into()]).unwrap();
        assert!(!r.success);
        assert_eq!(r.exit_code, Some(1));
    }

    #[test]
    fn captures_stdout() {
        let r = run_command(&["printf".into(), "hello".into()]).unwrap();
        assert_eq!(r.stdout, "hello");
    }

    #[test]
    fn missing_binary_is_error() {
        let err = run_command(&["definitely-not-a-binary-xyz-vet".into()]).unwrap_err();
        assert!(matches!(err, VetError::Spawn { .. }));
    }

    #[test]
    fn timeout_kills_hanging_command() {
        let opts = RunOptions {
            timeout: Some(Duration::from_millis(200)),
            output_cap: DEFAULT_OUTPUT_CAP,
        };
        let err = run_command_with(&["sleep".into(), "5".into()], &opts).unwrap_err();
        match err {
            VetError::Timeout { limit, .. } => {
                assert_eq!(limit, Duration::from_millis(200));
            }
            other => panic!("expected Timeout, got {other}"),
        }
    }

    #[test]
    fn output_cap_truncates() {
        // Print more than cap bytes.
        let opts = RunOptions {
            timeout: None,
            output_cap: 16,
        };
        let r = run_command_with(
            &["printf".into(), "abcdefghijklmnopqrstuvwxyz".into()],
            &opts,
        )
        .unwrap();
        assert!(r.stdout_truncated);
        assert_eq!(r.stdout.len(), 16);
    }
}
