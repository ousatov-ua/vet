//! Command-line interface definition.

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Output format for claim results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Format {
    /// Human-readable one-line results (default).
    #[default]
    Human,
    /// One JSON object per claim (JSON Lines).
    Jsonl,
}

/// When to colorize human output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ColorChoice {
    /// Color when stdout is a TTY and `NO_COLOR` is unset (default).
    #[default]
    Auto,
    /// Always color human output.
    Always,
    /// Never color.
    Never,
}

/// Claim checker for agent grounding.
///
/// Run one claim:
///   vclaim exit 0 -- cargo test
///   vclaim stdout contains ok -- ./tool
///   vclaim git clean
///
/// Batch:
///   vclaim -f claims.txt
///   cat claims.txt | vclaim
#[derive(Parser, Debug)]
#[command(
    name = "vclaim",
    version,
    about = "Claim checker for agent grounding",
    long_about = None,
    after_help = "\
EXAMPLES:
  vclaim exit 0 -- cargo test -q
  vclaim stdout contains ok -- ./tool
  vclaim json .status == \"healthy\" -- curl -sf localhost/health
  vclaim git clean
  vclaim files exist README.md
  vclaim env set CI
  vclaim duration lt 30s -- cargo test
  vclaim -f claims.txt
  vclaim --format jsonl exit 0 -- true
  vclaim --timeout 10s exit 0 -- ./slow-tool

FULL OUTPUT:
  Every run saves complete stdout/stderr to <temp>/vclaim-<hex>.txt
  (OS temp dir on Linux, macOS, Windows). Path is printed at the end.
  Read that file for full detail of what was tested — terminal shows
  pass/fail only.
"
)]
pub struct Cli {
    /// Output format: human (default) or jsonl.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,

    /// Colorize human output: auto (default), always, never.
    /// When auto, colors only if stdout is a TTY and NO_COLOR is unset.
    #[arg(long, value_enum, default_value_t = ColorChoice::Auto)]
    pub color: ColorChoice,

    /// Kill the claim command after this duration (e.g. 30s, 500ms, 2m).
    /// Default: no limit. Operational failure (exit 2) on timeout.
    #[arg(long, value_name = "DURATION")]
    pub timeout: Option<String>,

    /// Read claims from a file (one claim per line; `#` comments).
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Claim tokens and optional `--` + command.
    /// Examples: `exit 0 -- cargo test`, `git clean`, `files exist a b`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_builds() {
        Cli::command().debug_assert();
    }

    #[test]
    fn format_value_enum_parses() {
        let c = Cli::try_parse_from(["vclaim", "--format", "jsonl", "git", "clean"]).unwrap();
        assert_eq!(c.format, Format::Jsonl);
    }

    #[test]
    fn color_value_enum_parses() {
        let c = Cli::try_parse_from(["vclaim", "--color", "never", "git", "clean"]).unwrap();
        assert_eq!(c.color, ColorChoice::Never);
    }
}
