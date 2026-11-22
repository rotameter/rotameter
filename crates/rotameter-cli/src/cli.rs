#![forbid(unsafe_code)]
//! Top-level `clap` CLI definition, global options, and `serve` / ops arguments.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::cli_sub::{ImportArgs, KeyArgs, QueryArgs, RealtimeArgs, SiteArgs};
use crate::format::OutputFormat;

/// Unified headless Rotameter analytics CLI.
#[derive(Debug, Parser)]
#[command(
    name = "rotameter",
    version,
    about = "Privacy-first headless web analytics engine & operator CLI"
)]
pub struct Cli {
    /// Global endpoint, database, profile, and formatting options.
    #[command(flatten)]
    pub global: GlobalOpts,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Global options accepted across all `rotameter` subcommands.
#[derive(Debug, Default, Clone, Args)]
pub struct GlobalOpts {
    /// Remote Rotameter server base URL (`ROTAMETER_URL`).
    #[arg(long, global = true)]
    pub url: Option<String>,
    /// API Bearer token (`ROTAMETER_TOKEN`).
    #[arg(long, global = true)]
    pub token: Option<String>,
    /// Named profile in `~/.config/rotameter/config.toml` (`ROTAMETER_PROFILE`).
    #[arg(long, global = true)]
    pub profile: Option<String>,
    /// Cloudflare Access Service Token client ID (`CF_ACCESS_CLIENT_ID`).
    #[arg(long, global = true)]
    pub access_id: Option<String>,
    /// Cloudflare Access Service Token client secret (`CF_ACCESS_CLIENT_SECRET`).
    #[arg(long, global = true)]
    pub access_secret: Option<String>,
    /// Explicit path to `config.toml` (`ROTAMETER_CONFIG`).
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    /// Local `SQLite` database path (`ROTAMETER_DB`).
    #[arg(long, global = true)]
    pub db: Option<String>,
    /// Output format (`table`, `json`, or `csv`).
    #[arg(long, value_enum, global = true, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
    /// Suppress non-essential headers/status lines.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,
}

/// Top-level `rotameter` subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the HTTP ingest, query, and admin server.
    Serve(ServeArgs),
    /// Manage tracked sites (`upsert`, `list`, `show`, `purge`, `delete`).
    Site(SiteArgs),
    /// Manage scoped API keys (`create`, `list`, `revoke`).
    Key(KeyArgs),
    /// Run a composable analytics query against `/v1/query`.
    Query(QueryArgs),
    /// Show trailing 5-minute active visitors and 30-minute pulse.
    Realtime(RealtimeArgs),
    /// Backfill historical hits from `JSONL` or Nginx/Caddy CLF logs.
    Import(ImportArgs),
    /// Create a consistent online `SQLite VACUUM INTO` snapshot backup.
    Backup(BackupArgs),
    /// Verify local `SQLite` WAL health or remote `/v1/health` readiness.
    Doctor(DoctorArgs),
    /// Start the `stdio` Model Context Protocol server.
    Mcp,
    /// Launch the interactive `ratatui` flight-instrument dashboard.
    Tui,
}

/// Options for `rotameter serve`.
#[derive(Debug, Default, Clone, Args)]
pub struct ServeArgs {
    /// Socket address to bind (`ROTAMETER_BIND`, default `127.0.0.1:8000`).
    #[arg(long)]
    pub bind: Option<String>,
    /// Plaintext `rm_adm_...` admin token seeded idempotently on boot.
    #[arg(long)]
    pub bootstrap_admin_key: Option<String>,
    /// Public URL whose host seeds the default `self_site` slug.
    #[arg(long)]
    pub public_url: Option<String>,
    /// Disable local `self_site` first-boot initialization.
    #[arg(long)]
    pub no_self_site: bool,
    /// Explicitly enable or disable local `self_site` (`--self-site=false`).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_parser = clap::value_parser!(bool))]
    pub self_site: Option<bool>,
    /// Override the local `self_site` UID (`site_...`) or slug.
    #[arg(long)]
    pub self_site_id: Option<String>,
    /// `GeoIP` mode (`header` or `mmdb`).
    #[arg(long)]
    pub geoip: Option<String>,
    /// Path to `MaxMind` `.mmdb` database when `--geoip mmdb` is used.
    #[arg(long)]
    pub mmdb_path: Option<PathBuf>,
    /// Trust `CF-*` headers on unauthenticated sockets (`true` or `false`).
    #[arg(long, num_args = 0..=1, default_missing_value = "true", value_parser = clap::value_parser!(bool))]
    pub trusted_proxy: Option<bool>,
}

/// Options for `rotameter backup <dest>`.
#[derive(Debug, Clone, Args)]
pub struct BackupArgs {
    /// Destination file path for `SQLite VACUUM INTO`.
    pub dest: PathBuf,
}

/// Options for `rotameter doctor`.
#[derive(Debug, Default, Clone, Args)]
pub struct DoctorArgs {
    /// Force remote `/v1/health` verification.
    #[arg(long)]
    pub remote: bool,
    /// Force local `SQLite` WAL verification.
    #[arg(long)]
    pub local: bool,
}
