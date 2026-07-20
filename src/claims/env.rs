//! `env` claim: environment variable presence (values never printed).

use super::Verdict;
use std::ffi::OsString;

/// Claim: `env set NAME…` | `env !set NAME…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvClaim {
    /// When true, names must be set; when false, must be unset.
    pub should_be_set: bool,
    pub names: Vec<String>,
}

impl EnvClaim {
    pub fn display(&self) -> String {
        let op = if self.should_be_set { "set" } else { "!set" };
        format!("env {op} {}", self.names.join(" "))
    }

    pub fn evaluate(&self) -> Verdict {
        self.evaluate_with(|name| std::env::var_os(name))
    }

    /// Test hook; values are never included in evidence.
    pub fn evaluate_with<F>(&self, lookup: F) -> Verdict
    where
        F: Fn(&str) -> Option<OsString>,
    {
        let claim = self.display();
        if self.names.is_empty() {
            return Verdict::fail(claim, "no variable names provided");
        }

        for name in &self.names {
            let is_set = lookup(name).is_some();
            if self.should_be_set && !is_set {
                return Verdict::fail(claim, format!("{name} unset"));
            }
            if !self.should_be_set && is_set {
                return Verdict::fail(claim, format!("{name} set"));
            }
        }

        let n = self.names.len();
        let evidence = if self.should_be_set {
            format!("{n} var(s) set")
        } else {
            format!("{n} var(s) unset")
        };
        Verdict::pass(claim, evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn set_pass() {
        let map: HashMap<_, _> = [("PATH", OsString::from("/bin"))].into();
        let c = EnvClaim {
            should_be_set: true,
            names: vec!["PATH".into()],
        };
        let v = c.evaluate_with(|n| map.get(n).cloned());
        assert!(v.ok);
        assert!(!v.evidence.contains("/bin"));
    }

    #[test]
    fn set_fail_never_leaks_value() {
        let map: HashMap<_, _> = [("SECRET", OsString::from("s3cr3t"))].into();
        let c = EnvClaim {
            should_be_set: false,
            names: vec!["SECRET".into()],
        };
        let v = c.evaluate_with(|n| map.get(n).cloned());
        assert!(!v.ok);
        assert_eq!(v.evidence, "SECRET set");
        assert!(!v.evidence.contains("s3cr3t"));
    }

    #[test]
    fn empty_string_counts_as_set() {
        let map: HashMap<_, _> = [("EMPTY", OsString::new())].into();
        let c = EnvClaim {
            should_be_set: true,
            names: vec!["EMPTY".into()],
        };
        assert!(c.evaluate_with(|n| map.get(n).cloned()).ok);
    }
}
