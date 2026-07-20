//! `duration` claim: wall-clock bound on the command.

use super::Verdict;
use crate::run::RunResult;
use std::time::Duration;

/// Comparison operator (v0.1: `lt` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationOp {
    Lt,
}

/// Claim: `duration lt 30s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationClaim {
    pub op: DurationOp,
    pub limit: Duration,
}

impl DurationClaim {
    pub fn display(&self) -> String {
        let ms = self.limit.as_millis();
        let limit = if ms.is_multiple_of(60_000) && ms >= 60_000 {
            format!("{}m", ms / 60_000)
        } else if ms.is_multiple_of(1000) {
            format!("{}s", ms / 1000)
        } else {
            format!("{ms}ms")
        };
        match self.op {
            DurationOp::Lt => format!("duration lt {limit}"),
        }
    }

    pub fn evaluate(&self, run: &RunResult) -> Verdict {
        let claim = self.display();
        let took = Duration::from_millis(run.ms);
        let ok = match self.op {
            DurationOp::Lt => took < self.limit,
        };
        let evidence = format!("took {}ms (limit {}ms)", run.ms, self.limit.as_millis());
        if ok {
            Verdict::pass(claim, evidence).with_run(run)
        } else {
            Verdict::fail(claim, evidence).with_run(run)
        }
    }
}

/// Parse duration tokens like `30s`, `500ms`, `2m`.
pub fn parse_duration_token(token: &str) -> Result<Duration, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("empty duration".into());
    }

    let (num_part, unit) = if let Some(rest) = token.strip_suffix("ms") {
        (rest, "ms")
    } else if let Some(rest) = token.strip_suffix('s') {
        (rest, "s")
    } else if let Some(rest) = token.strip_suffix('m') {
        (rest, "m")
    } else {
        return Err(format!(
            "invalid duration `{token}` (use e.g. 500ms, 30s, 2m)"
        ));
    };

    let n: u64 = num_part
        .parse()
        .map_err(|_| format!("invalid duration magnitude `{num_part}`"))?;

    let d = match unit {
        "ms" => Duration::from_millis(n),
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(n.saturating_mul(60)),
        _ => unreachable!(),
    };
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_ms(ms: u64) -> RunResult {
        RunResult {
            exit_code: Some(0),
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            ms,
            command_display: "cmd".into(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn lt_pass() {
        let c = DurationClaim {
            op: DurationOp::Lt,
            limit: Duration::from_secs(30),
        };
        assert!(c.evaluate(&run_ms(100)).ok);
    }

    #[test]
    fn lt_fail() {
        let c = DurationClaim {
            op: DurationOp::Lt,
            limit: Duration::from_millis(50),
        };
        assert!(!c.evaluate(&run_ms(100)).ok);
    }

    #[test]
    fn parse_units() {
        assert_eq!(
            parse_duration_token("500ms").unwrap(),
            Duration::from_millis(500)
        );
        assert_eq!(
            parse_duration_token("30s").unwrap(),
            Duration::from_secs(30)
        );
        assert_eq!(
            parse_duration_token("2m").unwrap(),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn parse_rejects_bad() {
        assert!(parse_duration_token("30").is_err());
        assert!(parse_duration_token("xs").is_err());
    }
}
