//! `vet` CLI entrypoint.

use std::io::{self, Write};
use std::process::ExitCode as StdExitCode;
use vet::{run_from_args, ExitCode};

fn main() -> StdExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let code = run_from_args(std::env::args_os(), &mut stdout, &mut stderr);
    let _ = stdout.flush();
    let _ = stderr.flush();
    match code {
        ExitCode::Success => StdExitCode::from(0),
        ExitCode::ClaimFailed => StdExitCode::from(1),
        ExitCode::Error => StdExitCode::from(2),
    }
}
