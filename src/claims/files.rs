//! `files` claim: path existence checks.

use super::Verdict;
use std::path::{Path, PathBuf};

/// Claim: `files exist PATH…` | `files !exist PATH…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesClaim {
    /// When true, all paths must exist; when false, none may exist.
    pub should_exist: bool,
    pub paths: Vec<PathBuf>,
}

impl FilesClaim {
    pub fn display(&self) -> String {
        let op = if self.should_exist { "exist" } else { "!exist" };
        let paths = self
            .paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        format!("files {op} {paths}")
    }

    pub fn evaluate(&self) -> Verdict {
        self.evaluate_with(|p| p.exists())
    }

    /// Test hook with injectable existence check.
    pub fn evaluate_with<F>(&self, exists: F) -> Verdict
    where
        F: Fn(&Path) -> bool,
    {
        let claim = self.display();
        if self.paths.is_empty() {
            return Verdict::fail(claim, "no paths provided");
        }

        for path in &self.paths {
            let present = exists(path);
            if self.should_exist && !present {
                return Verdict::fail(claim, format!("{} missing", path.display()));
            }
            if !self.should_exist && present {
                return Verdict::fail(claim, format!("{} exists", path.display()));
            }
        }

        let n = self.paths.len();
        let evidence = if self.should_exist {
            format!("{n} path(s) exist")
        } else {
            format!("{n} path(s) absent")
        };
        Verdict::pass(claim, evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_exist() {
        let set: HashSet<PathBuf> = ["a", "b"].into_iter().map(PathBuf::from).collect();
        let c = FilesClaim {
            should_exist: true,
            paths: vec![PathBuf::from("a"), PathBuf::from("b")],
        };
        assert!(c.evaluate_with(|p| set.contains(p)).ok);
    }

    #[test]
    fn missing_path_fails() {
        let c = FilesClaim {
            should_exist: true,
            paths: vec![PathBuf::from("missing")],
        };
        let v = c.evaluate_with(|_| false);
        assert!(!v.ok);
        assert!(v.evidence.contains("missing"));
    }

    #[test]
    fn not_exist() {
        let c = FilesClaim {
            should_exist: false,
            paths: vec![PathBuf::from("ghost")],
        };
        assert!(c.evaluate_with(|_| false).ok);
        assert!(!c.evaluate_with(|_| true).ok);
    }
}
