//! `exit` claim: process exit code expectations.

use super::Verdict;
use crate::run::RunResult;

/// Expected process exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitExpect {
    /// Exact exit code (e.g. `0`, `101`).
    Code(i32),
    /// Any non-zero exit (including signal termination).
    Nonzero,
}

/// Claim: `exit 0` | `exit 101` | `exit nonzero`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitClaim {
    pub expect: ExitExpect,
}

impl ExitClaim {
    pub fn display(&self) -> String {
        match self.expect {
            ExitExpect::Code(n) => format!("exit {n}"),
            ExitExpect::Nonzero => "exit nonzero".into(),
        }
    }

    pub fn evaluate(&self, run: &RunResult) -> Verdict {
        let claim = self.display();
        let status = match run.exit_code {
            Some(code) => format!("exit {code}"),
            None => "terminated by signal".into(),
        };

        let ok = match self.expect {
            ExitExpect::Code(expected) => run.exit_code == Some(expected),
            ExitExpect::Nonzero => !run.success,
        };

        if ok {
            // On pass, prefer the command as evidence (agent-friendly).
            let evidence = if run.command_display.is_empty() {
                status
            } else {
                run.command_display.clone()
            };
            Verdict::pass(claim, evidence).with_run(run)
        } else {
            Verdict::fail(claim, status).with_run(run)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(exit: i32) -> RunResult {
        RunResult {
            exit_code: Some(exit),
            success: exit == 0,
            stdout: String::new(),
            stderr: String::new(),
            ms: 1,
            command_display: "cmd".into(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn exit_zero_pass() {
        let v = ExitClaim {
            expect: ExitExpect::Code(0),
        }
        .evaluate(&run(0));
        assert!(v.ok);
    }

    #[test]
    fn exit_zero_fail() {
        let v = ExitClaim {
            expect: ExitExpect::Code(0),
        }
        .evaluate(&run(1));
        assert!(!v.ok);
        assert_eq!(v.evidence, "exit 1");
    }

    #[test]
    fn exit_nonzero_pass() {
        let v = ExitClaim {
            expect: ExitExpect::Nonzero,
        }
        .evaluate(&run(7));
        assert!(v.ok);
    }

    #[test]
    fn exit_nonzero_fail_on_zero() {
        let v = ExitClaim {
            expect: ExitExpect::Nonzero,
        }
        .evaluate(&run(0));
        assert!(!v.ok);
    }

    #[test]
    fn exit_custom_code() {
        let v = ExitClaim {
            expect: ExitExpect::Code(101),
        }
        .evaluate(&run(101));
        assert!(v.ok);
    }
}
