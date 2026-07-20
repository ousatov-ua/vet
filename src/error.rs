//! Error types and process exit mapping.

use std::fmt;
use std::time::Duration;

/// Process exit codes for `vclaim`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// All claims passed.
    Success = 0,
    /// One or more claims failed.
    ClaimFailed = 1,
    /// Usage, parse, spawn, timeout, or other runtime error.
    Error = 2,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code as i32
    }
}

/// Fatal errors that stop evaluation (process exit 2).
#[derive(Debug)]
pub enum VclaimError {
    Usage(String),
    Parse(String),
    Spawn {
        command: String,
        source: std::io::Error,
    },
    /// Command exceeded `--timeout`.
    Timeout {
        command: String,
        limit: Duration,
    },
    Io(String),
}

impl fmt::Display for VclaimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VclaimError::Usage(msg) => write!(f, "usage error: {msg}"),
            VclaimError::Parse(msg) => write!(f, "parse error: {msg}"),
            VclaimError::Spawn { command, source } => {
                write!(f, "failed to spawn `{command}`: {source}")
            }
            VclaimError::Timeout { command, limit } => {
                write!(
                    f,
                    "command timed out after {}ms: `{command}`",
                    limit.as_millis()
                )
            }
            VclaimError::Io(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for VclaimError {}

impl From<std::io::Error> for VclaimError {
    fn from(err: std::io::Error) -> Self {
        VclaimError::Io(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VclaimError>;
