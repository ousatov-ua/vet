//! `git` claim: working tree clean/dirty via `git status --porcelain`.

use super::Verdict;
use crate::error::{Result, VclaimError};
use std::process::Command;

/// Claim: `git clean` | `git dirty`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitClaim {
    Clean,
    Dirty,
}

impl GitClaim {
    pub fn display(&self) -> String {
        match self {
            GitClaim::Clean => "git clean".into(),
            GitClaim::Dirty => "git dirty".into(),
        }
    }

    pub fn evaluate(&self) -> Result<Verdict> {
        self.evaluate_with(git_status)
    }

    /// Test hook with injectable status.
    pub fn evaluate_with<F>(&self, status_fn: F) -> Result<Verdict>
    where
        F: FnOnce() -> GitStatus,
    {
        let claim = self.display();
        match status_fn() {
            GitStatus::Clean => {
                let ok = matches!(self, GitClaim::Clean);
                Ok(if ok {
                    Verdict::pass(claim, "working tree clean")
                } else {
                    Verdict::fail(claim, "working tree clean")
                })
            }
            GitStatus::Dirty(n) => {
                let ok = matches!(self, GitClaim::Dirty);
                let evidence = format!("{n} dirty path(s)");
                Ok(if ok {
                    Verdict::pass(claim, evidence)
                } else {
                    Verdict::fail(claim, evidence)
                })
            }
            // Environmental fact: claim can still fail against it.
            GitStatus::NotRepo => Ok(Verdict::fail(claim, "not a git repository")),
            // Operational: tool missing or status invocation failed → exit 2.
            GitStatus::Error(msg) => Err(VclaimError::Io(format!("git: {msg}"))),
        }
    }
}

/// Result of probing the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStatus {
    Clean,
    Dirty(usize),
    NotRepo,
    Error(String),
}

fn git_status() -> GitStatus {
    let output = match Command::new("git").args(["status", "--porcelain"]).output() {
        Ok(o) => o,
        Err(e) => {
            return GitStatus::Error(format!("failed to run git: {e}"));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if stderr.contains("not a git repository") {
            return GitStatus::NotRepo;
        }
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return GitStatus::Error(if msg.is_empty() {
            format!("git status exited {}", output.status)
        } else {
            msg
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        GitStatus::Clean
    } else {
        GitStatus::Dirty(lines.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_pass() {
        let v = GitClaim::Clean.evaluate_with(|| GitStatus::Clean).unwrap();
        assert!(v.ok);
    }

    #[test]
    fn clean_fails_when_dirty() {
        let v = GitClaim::Clean
            .evaluate_with(|| GitStatus::Dirty(2))
            .unwrap();
        assert!(!v.ok);
        assert!(v.evidence.contains("2 dirty"));
    }

    #[test]
    fn dirty_pass() {
        let v = GitClaim::Dirty
            .evaluate_with(|| GitStatus::Dirty(1))
            .unwrap();
        assert!(v.ok);
    }

    #[test]
    fn not_repo_is_claim_fail() {
        let v = GitClaim::Clean
            .evaluate_with(|| GitStatus::NotRepo)
            .unwrap();
        assert!(!v.ok);
        assert!(v.evidence.contains("not a git repository"));
    }

    #[test]
    fn git_tool_error_is_operational() {
        let err = GitClaim::Clean
            .evaluate_with(|| GitStatus::Error("failed to run git: No such file".into()))
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("git:"));
        assert!(msg.contains("failed to run git"));
    }
}
