//! Error types and process exit mapping.

use std::fmt;
use std::time::Duration;

/// Process exit codes for `vet`.
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
pub enum VetError {
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

impl fmt::Display for VetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VetError::Usage(msg) => write!(f, "usage error: {msg}"),
            VetError::Parse(msg) => write!(f, "parse error: {msg}"),
            VetError::Spawn { command, source } => {
                write!(f, "failed to spawn `{command}`: {source}")
            }
            VetError::Timeout { command, limit } => {
                write!(
                    f,
                    "command timed out after {}ms: `{command}`",
                    limit.as_millis()
                )
            }
            VetError::Io(msg) => write!(f, "io error: {msg}"),
        }
    }
}

impl std::error::Error for VetError {}

impl From<std::io::Error> for VetError {
    fn from(err: std::io::Error) -> Self {
        VetError::Io(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VetError>;
