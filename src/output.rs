//! Human and JSONL formatters.

use crate::claims::Verdict;
use crate::cli::Format;
use std::io::{self, Write};

/// Write one verdict in the requested format.
pub fn write_verdict(
    out: &mut dyn Write,
    format: Format,
    verdict: &Verdict,
    color: bool,
) -> io::Result<()> {
    match format {
        Format::Human => write_human(out, verdict, color),
        Format::Jsonl => write_jsonl(out, verdict),
    }
}

fn write_human(out: &mut dyn Write, v: &Verdict, color: bool) -> io::Result<()> {
    let (mark, paint) = if v.ok {
        ("PASS", Color::Green)
    } else {
        ("FAIL", Color::Red)
    };

    let mark = if color {
        paint.paint(mark)
    } else {
        mark.to_string()
    };
    writeln!(out, "{mark}  {}  ({})", v.claim, v.evidence)?;
    Ok(())
}

fn write_jsonl(out: &mut dyn Write, v: &Verdict) -> io::Result<()> {
    let line = serde_json::to_string(v).map_err(io::Error::other)?;
    writeln!(out, "{line}")?;
    Ok(())
}

enum Color {
    Green,
    Red,
}

impl Color {
    fn paint(self, s: &str) -> String {
        let code = match self {
            Color::Green => "32",
            Color::Red => "31",
        };
        format!("\x1b[{code}m{s}\x1b[0m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ok: bool) -> Verdict {
        if ok {
            Verdict::pass("exit 0", "true")
        } else {
            Verdict::fail("exit 0", "exit 1")
        }
    }

    #[test]
    fn human_pass() {
        let mut buf = Vec::new();
        write_verdict(&mut buf, Format::Human, &sample(true), false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("PASS"));
        assert!(s.contains("exit 0"));
        assert!(!s.contains('\u{1b}'));
    }

    #[test]
    fn human_color_when_enabled() {
        let mut buf = Vec::new();
        write_verdict(&mut buf, Format::Human, &sample(true), true).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\x1b[32m"));
    }

    #[test]
    fn jsonl_shape() {
        let mut buf = Vec::new();
        write_verdict(&mut buf, Format::Jsonl, &sample(false), false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["claim"], "exit 0");
        assert_eq!(v["evidence"], "exit 1");
    }
}
