//! Full-run transcript saved under the OS temp directory.
//!
//! Terminal output is a short pass/fail summary. Complete stdout/stderr of
//! tested commands (plus verdicts) is written to a uniquely named file so
//! humans and agents can inspect what was actually observed.

use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::claims::Verdict;
use crate::error::{Result, VclaimError};
use crate::run::RunResult;

/// One evaluated job's data for the transcript.
pub struct TranscriptJob<'a> {
    pub verdict: &'a Verdict,
    /// Present when a command was executed for this claim.
    pub run: Option<&'a RunResult>,
}

/// Write a full transcript and return its path under the OS temp dir.
///
/// Filename pattern: `vclaim-<unique-hex>.txt` in [`std::env::temp_dir`].
pub fn write_transcript(jobs: &[TranscriptJob<'_>]) -> Result<PathBuf> {
    let dir = std::env::temp_dir();
    let mut last_err: Option<std::io::Error> = None;

    for attempt in 0u32..32 {
        let path = dir.join(format!("vclaim-{}.txt", unique_id(attempt)));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                write_body(&mut file, jobs).map_err(|e| VclaimError::Io(e.to_string()))?;
                file.flush().map_err(|e| VclaimError::Io(e.to_string()))?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = Some(e);
                continue;
            }
            Err(e) => return Err(VclaimError::Io(e.to_string())),
        }
    }

    Err(VclaimError::Io(
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| {
                "could not create unique vclaim transcript in temp directory".into()
            }),
    ))
}

fn unique_id(attempt: u32) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    nanos.hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    attempt.hash(&mut hasher);
    // Stack address adds process-local entropy without extra crates.
    let stack_marker = &nanos as *const u128 as usize;
    stack_marker.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    format!("{:016x}{:08x}", hasher.finish(), std::process::id())
}

fn write_body(w: &mut impl Write, jobs: &[TranscriptJob<'_>]) -> std::io::Result<()> {
    writeln!(
        w,
        "vclaim full output\n\
         ===============\n\
         Terminal output shows pass/fail only. This file has complete\n\
         stdout/stderr of commands tested by vclaim, plus claim verdicts.\n\
         Humans and agents should read this file for full detail.\n"
    )?;

    for (i, job) in jobs.iter().enumerate() {
        writeln!(w, "--- claim {} ---", i + 1)?;
        writeln!(w, "claim: {}", job.verdict.claim)?;
        writeln!(
            w,
            "ok: {}",
            if job.verdict.ok { "true" } else { "false" }
        )?;
        writeln!(w, "evidence: {}", job.verdict.evidence)?;

        if let Some(code) = job.verdict.exit {
            writeln!(w, "exit: {code}")?;
        }
        if let Some(ms) = job.verdict.ms {
            writeln!(w, "duration_ms: {ms}")?;
        }

        if let Some(run) = job.run {
            writeln!(w, "command: {}", run.command_display)?;
            writeln!(w)?;
            writeln!(w, "=== stdout ===")?;
            write_stream(w, &run.stdout)?;
            if run.stdout_truncated {
                writeln!(w, "\n[stdout truncated at capture cap]")?;
            }
            writeln!(w)?;
            writeln!(w, "=== stderr ===")?;
            write_stream(w, &run.stderr)?;
            if run.stderr_truncated {
                writeln!(w, "\n[stderr truncated at capture cap]")?;
            }
            writeln!(w)?;
        } else {
            writeln!(w, "(no command — workspace/env claim)")?;
        }
        writeln!(w)?;
    }

    Ok(())
}

fn write_stream(w: &mut impl Write, text: &str) -> std::io::Result<()> {
    write!(w, "{text}")?;
    if !text.is_empty() && !text.ends_with('\n') {
        writeln!(w)?;
    }
    Ok(())
}

/// Footer line(s) pointing at the transcript path (human or JSONL).
pub fn write_footer(w: &mut dyn Write, path: &Path, jsonl: bool) -> Result<()> {
    let path_str = path.display().to_string();
    if jsonl {
        let rec = serde_json::json!({
            "log": path_str,
        });
        writeln!(w, "{rec}").map_err(|e| VclaimError::Io(e.to_string()))?;
    } else {
        writeln!(w).map_err(|e| VclaimError::Io(e.to_string()))?;
        writeln!(w, "Log: {path_str}").map_err(|e| VclaimError::Io(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::Verdict;
    use crate::run::RunResult;

    fn sample_run(stdout: &str, stderr: &str) -> RunResult {
        RunResult {
            exit_code: Some(0),
            success: true,
            stdout: stdout.into(),
            stderr: stderr.into(),
            ms: 5,
            command_display: "printf hello-out".into(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    #[test]
    fn filename_is_vclaim_prefix_txt() {
        let v = Verdict::pass("exit 0", "exit 0");
        let jobs = [TranscriptJob {
            verdict: &v,
            run: None,
        }];
        let path = write_transcript(&jobs).expect("write");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("vclaim-"), "name={name}");
        assert!(name.ends_with(".txt"), "name={name}");
        let body = std::fs::read_to_string(&path).expect("read back");
        assert!(body.contains("vclaim full output"));
        assert!(body.contains("claim: exit 0"));
        assert!(body.contains("ok: true"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn transcript_includes_stdout_stderr() {
        let run = sample_run("hello-out", "hello-err");
        let v = Verdict::pass("stdout contains hello-out", "found");
        let jobs = [TranscriptJob {
            verdict: &v,
            run: Some(&run),
        }];
        let path = write_transcript(&jobs).expect("write");
        let body = std::fs::read_to_string(&path).expect("read");
        assert!(body.contains("hello-out"));
        assert!(body.contains("hello-err"));
        assert!(body.contains("command: printf hello-out"));
        assert!(body.contains("=== stdout ==="));
        assert!(body.contains("=== stderr ==="));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn footer_human_mentions_path() {
        let mut buf = Vec::new();
        let path = PathBuf::from("/tmp/vclaim-deadbeef.txt");
        write_footer(&mut buf, &path, false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Log:"));
        assert!(s.contains("/tmp/vclaim-deadbeef.txt"));
    }

    #[test]
    fn footer_jsonl_is_json_with_path() {
        let mut buf = Vec::new();
        let path = PathBuf::from("/tmp/vclaim-cafe.txt");
        write_footer(&mut buf, &path, true).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let v: serde_json::Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["log"], "/tmp/vclaim-cafe.txt");
        assert!(v.get("note").is_none());
    }
}
