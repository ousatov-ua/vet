//! `json` claim: jq-lite path checks on command stdout.

use super::Verdict;
use crate::run::RunResult;
use crate::util::truncate_chars;
use serde_json::Value;

/// JSON comparison mode.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonOp {
    /// Path exists and value is truthy (not null/false/0/""/[]/{}).
    Truthy,
    /// Path present (null counts as exists).
    Exists,
    /// Deep equality against a JSON value.
    Equals(Value),
}

/// Claim: `json PATH` | `json PATH exists` | `json PATH == VALUE`.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonClaim {
    pub path: String,
    pub op: JsonOp,
}

impl JsonClaim {
    pub fn display(&self) -> String {
        match &self.op {
            JsonOp::Truthy => format!("json {}", format_path(&self.path)),
            JsonOp::Exists => format!("json {} exists", format_path(&self.path)),
            JsonOp::Equals(v) => format!("json {} == {}", format_path(&self.path), v),
        }
    }

    pub fn evaluate(&self, run: &RunResult) -> Verdict {
        let claim = self.display();
        let trimmed = run.stdout.trim();
        if trimmed.is_empty() {
            return Verdict::fail(claim, "empty stdout").with_run(run);
        }

        let root: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                return Verdict::fail(claim, format!("invalid json: {e}")).with_run(run);
            }
        };

        let resolved = resolve_path(&root, &self.path);
        match &self.op {
            JsonOp::Truthy => match resolved {
                PathResult::Missing => {
                    Verdict::fail(claim, format!("path {} missing", format_path(&self.path)))
                        .with_run(run)
                }
                PathResult::Found(v) if is_truthy(v) => {
                    Verdict::pass(claim, compact(v)).with_run(run)
                }
                PathResult::Found(v) => {
                    Verdict::fail(claim, format!("not truthy: {}", compact(v))).with_run(run)
                }
            },
            JsonOp::Exists => match resolved {
                PathResult::Missing => {
                    Verdict::fail(claim, format!("path {} missing", format_path(&self.path)))
                        .with_run(run)
                }
                PathResult::Found(v) => {
                    Verdict::pass(claim, format!("exists: {}", compact(v))).with_run(run)
                }
            },
            JsonOp::Equals(expected) => match resolved {
                PathResult::Missing => {
                    Verdict::fail(claim, format!("path {} missing", format_path(&self.path)))
                        .with_run(run)
                }
                PathResult::Found(v) if v == expected => {
                    Verdict::pass(claim, compact(v)).with_run(run)
                }
                PathResult::Found(v) => {
                    Verdict::fail(claim, format!("got {}, expected {}", compact(v), expected))
                        .with_run(run)
                }
            },
        }
    }
}

enum PathResult<'a> {
    Found(&'a Value),
    Missing,
}

/// Walk a dotted path: `status`, `.status`, `items.0.name`.
fn resolve_path<'a>(root: &'a Value, path: &str) -> PathResult<'a> {
    let path = path.strip_prefix('.').unwrap_or(path);
    if path.is_empty() {
        return PathResult::Found(root);
    }

    let mut current = root;
    for seg in path.split('.') {
        if seg.is_empty() {
            return PathResult::Missing;
        }
        match current {
            Value::Object(map) => match map.get(seg) {
                Some(v) => current = v,
                None => return PathResult::Missing,
            },
            Value::Array(arr) => {
                let idx: usize = match seg.parse() {
                    Ok(i) => i,
                    Err(_) => return PathResult::Missing,
                };
                match arr.get(idx) {
                    Some(v) => current = v,
                    None => return PathResult::Missing,
                }
            }
            _ => return PathResult::Missing,
        }
    }
    PathResult::Found(current)
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn compact(v: &Value) -> String {
    truncate_chars(&v.to_string(), 80)
}

fn format_path(path: &str) -> String {
    if path.is_empty() || path.starts_with('.') {
        path.to_string()
    } else {
        format!(".{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_json(body: &str) -> RunResult {
        RunResult {
            exit_code: Some(0),
            success: true,
            stdout: body.into(),
            stderr: String::new(),
            ms: 1,
            command_display: "cmd".into(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn truthy_ok() {
        let c = JsonClaim {
            path: "ok".into(),
            op: JsonOp::Truthy,
        };
        assert!(c.evaluate(&run_json(r#"{"ok":true}"#)).ok);
    }

    #[test]
    fn truthy_false() {
        let c = JsonClaim {
            path: "ok".into(),
            op: JsonOp::Truthy,
        };
        assert!(!c.evaluate(&run_json(r#"{"ok":false}"#)).ok);
    }

    #[test]
    fn exists_null() {
        let c = JsonClaim {
            path: "x".into(),
            op: JsonOp::Exists,
        };
        assert!(c.evaluate(&run_json(r#"{"x":null}"#)).ok);
    }

    #[test]
    fn equals_string() {
        let c = JsonClaim {
            path: "status".into(),
            op: JsonOp::Equals(json!("healthy")),
        };
        assert!(c.evaluate(&run_json(r#"{"status":"healthy"}"#)).ok);
    }

    #[test]
    fn missing_path() {
        let c = JsonClaim {
            path: "status".into(),
            op: JsonOp::Truthy,
        };
        let v = c.evaluate(&run_json(r#"{"other":1}"#));
        assert!(!v.ok);
        assert!(v.evidence.contains("missing"));
    }

    #[test]
    fn array_index() {
        let c = JsonClaim {
            path: "items.0.name".into(),
            op: JsonOp::Equals(json!("a")),
        };
        assert!(c.evaluate(&run_json(r#"{"items":[{"name":"a"}]}"#)).ok);
    }

    #[test]
    fn invalid_json() {
        let c = JsonClaim {
            path: "x".into(),
            op: JsonOp::Truthy,
        };
        let v = c.evaluate(&run_json("not-json"));
        assert!(!v.ok);
        assert!(v.evidence.contains("invalid json"));
    }

    #[test]
    fn resolve_root() {
        let root = json!({"a": 1});
        match resolve_path(&root, "") {
            PathResult::Found(v) => assert_eq!(v, &root),
            _ => panic!("expected root"),
        }
    }
}
