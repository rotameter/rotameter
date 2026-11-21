#![forbid(unsafe_code)]
//! Unified `rotameter` CLI binary entrypoint.

use std::process::ExitCode;

fn main() -> ExitCode {
    println!("{}", rotameter_cli::version_banner());
    ExitCode::SUCCESS
}
