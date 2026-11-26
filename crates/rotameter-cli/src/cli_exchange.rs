#![forbid(unsafe_code)]
//! CLI argument definitions for `rotameter backup --format portable`, `restore`, and `migrate`.

use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::format::OutputFormat;

/// Subcommand-level `--format <table|json|csv>` override propagated to nested subcommands.
#[derive(Debug, Default, Clone, Args)]
pub struct OutputFmtArg {
    /// Output format (`table`, `json`, or `csv`).
    #[arg(id = "output_format", long = "format", value_enum, global = true)]
    pub format: Option<OutputFormat>,
}

/// Output container format supported by `rotameter backup`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum BackupFormat {
    /// Native online `SQLite VACUUM INTO` snapshot file (default).
    #[default]
    Sqlite,
    /// Portable Arrow/Parquet directory (`manifest.json` + `control_plane.json` + `.parquet`).
    Portable,
}

/// Plane selector supported by `rotameter restore` and `rotameter migrate`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum ExchangePlane {
    /// Transfer both control plane (`orgs`, `sites`, `api_keys`, `rotating_salts`) and analytics data plane.
    #[default]
    All,
    /// Transfer only control-plane metadata.
    Control,
    /// Transfer only analytics data (`events`, `sessions`, `rollups_*`).
    Analytics,
}

impl ExchangePlane {
    /// Returns `true` when control-plane state should be transferred.
    pub const fn includes_control(self) -> bool {
        matches!(self, Self::All | Self::Control)
    }

    /// Returns `true` when analytics data-plane state should be transferred.
    pub const fn includes_analytics(self) -> bool {
        matches!(self, Self::All | Self::Analytics)
    }

    /// Returns the lowercase wire label (`all`, `control`, `analytics`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Control => "control",
            Self::Analytics => "analytics",
        }
    }
}

/// Options for `rotameter backup <dest>`.
#[derive(Debug, Clone, Args)]
pub struct BackupArgs {
    /// Destination file or directory path (ends in `.tar.zst` to emit a compressed archive).
    pub dest: PathBuf,
    /// Snapshot format (`sqlite` via `VACUUM INTO`, or `portable` Arrow/Parquet bundle).
    #[arg(long, value_enum, default_value_t = BackupFormat::Sqlite)]
    pub format: BackupFormat,
    /// Pack a `--format portable` export into a single `.tar.zst` archive.
    #[arg(long)]
    pub archive: bool,
}

/// Options for `rotameter restore <source>`.
#[derive(Debug, Clone, Args)]
pub struct RestoreArgs {
    /// Path to a portable backup directory or `.tar.zst` archive.
    pub source: PathBuf,
    /// Target database DSN (`SQLite` path/`sqlite://...` or `DuckDB`).
    #[arg(long)]
    pub to: Option<String>,
    /// Data planes to restore (`all`, `control`, or `analytics`).
    #[arg(long, value_enum, default_value_t = ExchangePlane::All)]
    pub plane: ExchangePlane,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}

/// Options for `rotameter migrate --from <dsn> --to <dsn>`.
#[derive(Debug, Clone, Args)]
pub struct MigrateArgs {
    /// Source database DSN (bare path or `sqlite://...`).
    #[arg(long)]
    pub from: String,
    /// Destination database DSN (bare path or `sqlite://...`).
    #[arg(long)]
    pub to: String,
    /// Data planes to migrate (`all`, `control`, or `analytics`).
    #[arg(long, value_enum, default_value_t = ExchangePlane::All)]
    pub plane: ExchangePlane,
    /// Optional path to write/stage an intermediate portable backup during migration.
    #[arg(long)]
    pub stage_dir: Option<PathBuf>,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}
