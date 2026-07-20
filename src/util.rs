//! Shared helpers.

/// Truncate `s` to at most `max_chars` Unicode scalar values.
/// Appends `…` when truncated. Safe on multi-byte UTF-8 (no byte slicing).
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_unchanged() {
        assert_eq!(truncate_chars("hi", 80), "hi");
    }

    #[test]
    fn exact_limit_unchanged() {
        let s: String = "a".repeat(80);
        assert_eq!(truncate_chars(&s, 80), s);
    }

    #[test]
    fn multibyte_no_panic() {
        let s: String = "é".repeat(81);
        let t = truncate_chars(&s, 80);
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 81); // 80 + ellipsis
                                           // Must not panic and must be valid UTF-8 (String guarantee).
        assert!(t.is_char_boundary(t.len()));
    }

    #[test]
    fn empty() {
        assert_eq!(truncate_chars("", 80), "");
    }
}
