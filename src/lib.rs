//! `vclaim` — claim checker for agent grounding.

mod claims;
mod cli;
mod error;
mod output;
mod parse;
mod run;
mod transcript;
mod util;

pub use claims::{
    evaluate, Claim, CommandPolicy, DurationClaim, DurationOp, EnvClaim, ExitClaim, ExitExpect,
    FilesClaim, GitClaim, JsonClaim, JsonOp, StreamClaim, StreamKind, StreamOp, Verdict,
};
pub use cli::{Cli, ColorChoice, Format};
pub use error::{ExitCode, Result, VclaimError};
pub use parse::{parse_batch, parse_line, ClaimJob};
pub use run::{run_command, run_command_with, RunOptions, RunResult, DEFAULT_OUTPUT_CAP};

use claims::parse_duration_token;
use clap::Parser;
use output::write_verdict;
use parse::collect_jobs;
use std::io::{self, Write};
use std::time::Duration;
use transcript::{write_footer, write_transcript, TranscriptJob};

/// Entry used by the binary and integration tests.
pub fn run_from_args<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            // clap already formats help/usage; map to exit codes.
            let _ = e.print();
            return if e.use_stderr() {
                // Error or `--help`/`--version` go through here differently.
                if e.kind() == clap::error::ErrorKind::DisplayHelp
                    || e.kind() == clap::error::ErrorKind::DisplayVersion
                {
                    ExitCode::Success
                } else {
                    ExitCode::Error
                }
            } else {
                ExitCode::Success
            };
        }
    };

    let color = resolve_color(cli.color);
    let timeout = match parse_timeout_flag(cli.timeout.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            let _ = writeln!(err, "vclaim: {e}");
            return ExitCode::Error;
        }
    };

    let jobs = match collect_jobs(&cli) {
        Ok(j) => j,
        Err(e) => {
            let _ = writeln!(err, "vclaim: {e}");
            return ExitCode::Error;
        }
    };

    match run_jobs(&jobs, cli.format, color, timeout, out, err) {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(err, "vclaim: {e}");
            ExitCode::Error
        }
    }
}

fn parse_timeout_flag(raw: Option<&str>) -> Result<Option<Duration>> {
    match raw {
        None => Ok(None),
        Some(s) => parse_duration_token(s).map(Some).map_err(VclaimError::Usage),
    }
}

fn resolve_color(choice: ColorChoice) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            use std::io::IsTerminal;
            io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
        }
    }
}

/// Run parsed claim jobs; write verdicts; save full transcript; return exit code.
pub fn run_jobs(
    jobs: &[ClaimJob],
    format: Format,
    color: bool,
    timeout: Option<Duration>,
    out: &mut dyn Write,
    _err: &mut dyn Write,
) -> Result<ExitCode> {
    let opts = RunOptions {
        timeout,
        output_cap: DEFAULT_OUTPUT_CAP,
    };
    let mut all_ok = true;
    let mut outcomes: Vec<(Verdict, Option<RunResult>)> = Vec::with_capacity(jobs.len());

    for job in jobs {
        let (verdict, run) = evaluate_job(job, &opts)?;
        if !verdict.ok {
            all_ok = false;
        }
        write_verdict(out, format, &verdict, color)?;
        outcomes.push((verdict, run));
    }

    let transcript_jobs: Vec<TranscriptJob<'_>> = outcomes
        .iter()
        .map(|(verdict, run)| TranscriptJob {
            verdict,
            run: run.as_ref(),
        })
        .collect();
    let path = write_transcript(&transcript_jobs)?;
    let jsonl = matches!(format, Format::Jsonl);
    write_footer(out, &path, jsonl)?;

    Ok(if all_ok {
        ExitCode::Success
    } else {
        ExitCode::ClaimFailed
    })
}

fn evaluate_job(job: &ClaimJob, opts: &RunOptions) -> Result<(Verdict, Option<RunResult>)> {
    match &job.command {
        Some(argv) => {
            let run = run_command_with(argv, opts)?;
            let verdict = evaluate(&job.claim, Some(&run))?;
            Ok((verdict, Some(run)))
        }
        None => {
            let verdict = evaluate(&job.claim, None)?;
            Ok((verdict, None))
        }
    }
}
