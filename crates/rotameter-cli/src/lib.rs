#![forbid(unsafe_code)]
//! Unified feature-gated CLI library for Rotameter.

pub mod cli;
pub mod cli_exchange;
pub mod cli_sub;
mod cmd_import;
mod cmd_key;
mod cmd_ops;
mod cmd_portable;
mod cmd_query;
mod cmd_realtime;
pub mod cmd_seed;
mod cmd_site;
mod cmd_site_ops;
mod context;
pub mod exit;
pub mod format;
mod import_caddy;
mod import_file;
mod import_filter;
mod import_follow;
mod import_parse;
mod import_socket;
mod import_stream;
#[cfg(feature = "sqlite")]
mod local_import;
#[cfg(feature = "sqlite")]
mod local_import_norm;
#[cfg(feature = "sqlite")]
mod local_keys;
#[cfg(feature = "sqlite")]
mod local_sites;
#[cfg(all(feature = "sqlite", feature = "arrow"))]
mod portable_export;
#[cfg(all(feature = "sqlite", feature = "arrow"))]
mod portable_import;
#[cfg(all(feature = "sqlite", feature = "arrow"))]
mod portable_migrate;
#[cfg(feature = "sqlite")]
mod seed_catalog;
#[cfg(feature = "sqlite")]
mod seed_gen;
#[cfg(feature = "sqlite")]
mod seed_rng;
#[cfg(feature = "sqlite")]
mod seed_session;
pub mod serve_cmd;
mod serve_parse;
mod serve_resolve;
#[cfg(all(feature = "server", feature = "sqlite"))]
pub mod serve_store;
pub mod storage_dsn;

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;
use rotameter_core::ComponentInfo;

use cli::{Cli, Command};
use exit::CliError;

/// Parses CLI arguments and executes the selected subcommand, returning a documented exit code.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let parsed = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let code = u8::try_from(err.exit_code()).unwrap_or(exit::EXIT_USAGE);
            let _ = err.print();
            return ExitCode::from(code);
        }
    };

    match dispatch(parsed) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err.message);
            ExitCode::from(err.code)
        }
    }
}

fn dispatch(mut parsed: Cli) -> Result<(), CliError> {
    if let Some(fmt) = subcommand_output_format(&parsed.command) {
        parsed.global.format = fmt;
    }
    let g = &parsed.global;
    match parsed.command {
        Command::Serve(args) => serve_cmd::run_serve(g, &args),
        Command::Site(args) => cmd_site::run_site(g, &args),
        Command::Key(args) => cmd_key::run_key(g, &args),
        Command::Query(args) => cmd_query::run_query(g, &args),
        Command::Realtime(args) => cmd_realtime::run_realtime(g, &args),
        Command::Import(args) => cmd_import::run_import(g, &args),
        Command::Seed(args) => cmd_seed::run_seed(g, &args),
        Command::Backup(args) => cmd_ops::run_backup(g, &args),
        Command::Restore(args) => cmd_portable::run_restore(g, &args),
        Command::Migrate(args) => cmd_portable::run_migrate(&args),
        Command::Doctor(args) => cmd_ops::run_doctor(g, &args),
        Command::Mcp => run_mcp(g),
        Command::Tui => run_tui(g),
    }
}

const fn subcommand_output_format(cmd: &Command) -> Option<format::OutputFormat> {
    match cmd {
        Command::Site(a) => a.output.format,
        Command::Key(a) => a.output.format,
        Command::Query(a) => a.output.format,
        Command::Realtime(a) => a.output.format,
        Command::Import(a) => a.output.format,
        Command::Seed(a) => a.output.format,
        Command::Restore(a) => a.output.format,
        Command::Migrate(a) => a.output.format,
        Command::Doctor(a) => a.output.format,
        Command::Serve(_) | Command::Backup(_) | Command::Mcp | Command::Tui => None,
    }
}

#[cfg(feature = "mcp")]
fn run_mcp(global: &cli::GlobalOpts) -> Result<(), CliError> {
    let client = context::remote_client(global)?;
    rotameter_mcp::run_stdio(&client).map_err(|err| CliError::runtime(err.to_string()))
}

#[cfg(not(feature = "mcp"))]
fn run_mcp(_global: &cli::GlobalOpts) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter mcp` requires the `mcp` cargo feature",
    ))
}

#[cfg(feature = "tui")]
fn run_tui(global: &cli::GlobalOpts) -> Result<(), CliError> {
    let client = context::remote_client(global)?;
    rotameter_tui::run_tui(&client).map_err(|err| CliError::runtime(err.to_string()))
}

#[cfg(not(feature = "tui"))]
fn run_tui(_global: &cli::GlobalOpts) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter tui` requires the `tui` cargo feature",
    ))
}

/// Returns metadata for all workspace components compiled under the active feature set.
pub fn enabled_components() -> Vec<ComponentInfo> {
    let mut components = vec![
        rotameter_core::component_info(),
        rotameter_client::component_info(),
        #[cfg(feature = "arrow")]
        rotameter_arrow::component_info(),
        #[cfg(feature = "server")]
        rotameter_server::component_info(),
        #[cfg(feature = "sqlite")]
        rotameter_store_sqlite::component_info(),
        #[cfg(feature = "mcp")]
        rotameter_mcp::component_info(),
        #[cfg(feature = "tui")]
        rotameter_tui::component_info(),
    ];
    let _ = &mut components;
    components
}

/// Formats the CLI version banner listing enabled workspace components.
pub fn version_banner() -> String {
    let names: Vec<&'static str> = enabled_components().into_iter().map(|c| c.name).collect();
    format!(
        "rotameter {} ({})",
        rotameter_core::WORKSPACE_VERSION,
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_components_match_workspace_version() {
        let components = enabled_components();
        let names: Vec<&str> = components.iter().map(|c| c.name).collect();

        assert!(names.contains(&"rotameter-core"));
        assert!(names.contains(&"rotameter-client"));
        #[cfg(feature = "server")]
        assert!(names.contains(&"rotameter-server"));
        #[cfg(feature = "sqlite")]
        assert!(names.contains(&"rotameter-store-sqlite"));
        #[cfg(feature = "mcp")]
        assert!(names.contains(&"rotameter-mcp"));
        #[cfg(feature = "tui")]
        assert!(names.contains(&"rotameter-tui"));

        for component in components {
            assert_eq!(component.version, rotameter_core::WORKSPACE_VERSION);
        }
    }

    #[test]
    fn version_banner_lists_core_and_client() {
        let banner = version_banner();
        assert!(banner.starts_with("rotameter "));
        assert!(banner.contains("rotameter-core"));
        assert!(banner.contains("rotameter-client"));
    }
}
