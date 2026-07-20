//! Claim kinds and evaluation.

mod duration;
mod env;
mod exit;
mod files;
mod git;
mod json;
mod stream;

pub use duration::{parse_duration_token, DurationClaim, DurationOp};
pub use env::EnvClaim;
pub use exit::{ExitClaim, ExitExpect};
pub use files::FilesClaim;
pub use git::GitClaim;
pub use json::{JsonClaim, JsonOp};
pub use stream::{StreamClaim, StreamKind, StreamOp};

use crate::error::{Result, VetError};
use crate::run::RunResult;
use serde::Serialize;

/// Whether a claim may / must be paired with a command after `--`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPolicy {
    /// Command required (`exit`, streams, `json`, `duration`).
    Required,
    /// Command forbidden (`files`, `env`, `git`).
    Forbidden,
}

/// A parsed claim ready for evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Claim {
    Exit(ExitClaim),
    Stream(StreamClaim),
    Json(JsonClaim),
    Files(FilesClaim),
    Env(EnvClaim),
    Git(GitClaim),
    Duration(DurationClaim),
}

impl Claim {
    /// Human-readable claim text (stable for JSONL `claim` field).
    pub fn display(&self) -> String {
        match self {
            Claim::Exit(c) => c.display(),
            Claim::Stream(c) => c.display(),
            Claim::Json(c) => c.display(),
            Claim::Files(c) => c.display(),
            Claim::Env(c) => c.display(),
            Claim::Git(c) => c.display(),
            Claim::Duration(c) => c.display(),
        }
    }

    /// Command policy for this claim kind.
    pub fn command_policy(&self) -> CommandPolicy {
        match self {
            Claim::Exit(_) | Claim::Stream(_) | Claim::Json(_) | Claim::Duration(_) => {
                CommandPolicy::Required
            }
            Claim::Files(_) | Claim::Env(_) | Claim::Git(_) => CommandPolicy::Forbidden,
        }
    }

    /// Whether this claim requires a command after `--`.
    pub fn requires_command(&self) -> bool {
        self.command_policy() == CommandPolicy::Required
    }

    /// Whether this claim rejects an accompanying command.
    pub fn rejects_command(&self) -> bool {
        self.command_policy() == CommandPolicy::Forbidden
    }
}

/// Outcome of evaluating one claim.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Verdict {
    pub claim: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<u64>,
    pub evidence: String,
}

impl Verdict {
    pub fn pass(claim: impl Into<String>, evidence: impl Into<String>) -> Self {
        Self {
            claim: claim.into(),
            ok: true,
            exit: None,
            ms: None,
            evidence: evidence.into(),
        }
    }

    pub fn fail(claim: impl Into<String>, evidence: impl Into<String>) -> Self {
        Self {
            claim: claim.into(),
            ok: false,
            exit: None,
            ms: None,
            evidence: evidence.into(),
        }
    }

    pub fn with_run(mut self, run: &RunResult) -> Self {
        self.exit = run.exit_code;
        self.ms = Some(run.ms);
        self
    }
}

/// Evaluate a claim, optionally against a command result.
///
/// Returns `Err` for internal contract violations (missing run for command
/// claims) and for operational failures (e.g. `git` tool unavailable).
pub fn evaluate(claim: &Claim, run: Option<&RunResult>) -> Result<Verdict> {
    match claim {
        Claim::Exit(c) => Ok(c.evaluate(require_run(claim, run)?)),
        Claim::Stream(c) => Ok(c.evaluate(require_run(claim, run)?)),
        Claim::Json(c) => Ok(c.evaluate(require_run(claim, run)?)),
        Claim::Duration(c) => Ok(c.evaluate(require_run(claim, run)?)),
        Claim::Files(c) => Ok(c.evaluate()),
        Claim::Env(c) => Ok(c.evaluate()),
        Claim::Git(c) => c.evaluate(),
    }
}

fn require_run<'a>(claim: &Claim, run: Option<&'a RunResult>) -> Result<&'a RunResult> {
    run.ok_or_else(|| {
        VetError::Usage(format!(
            "internal: claim `{}` requires a command result",
            claim.display()
        ))
    })
}
