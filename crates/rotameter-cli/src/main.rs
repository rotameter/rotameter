#![forbid(unsafe_code)]
//! Unified `rotameter` CLI binary entrypoint.

use std::process::ExitCode;

fn main() -> ExitCode {
    rotameter_cli::run(std::env::args_os())
}
