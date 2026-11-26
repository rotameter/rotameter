#![forbid(unsafe_code)]
//! `clap` argument definitions for `site`, `key`, `query`, `realtime`, and `import`.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::cli_exchange::OutputFmtArg;

/// `rotameter site` subcommand tree.
#[derive(Debug, Args)]
pub struct SiteArgs {
    /// Site operation to run.
    #[command(subcommand)]
    pub action: SiteCommand,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}

/// Site management actions.
#[derive(Debug, Subcommand)]
pub enum SiteCommand {
    /// Idempotently create or update a site by `site_...` ID or slug.
    Upsert {
        /// Target site identifier (`site_...` or `slug`).
        site: String,
        /// Updated org-unique slug.
        #[arg(long)]
        slug: Option<String>,
        /// Human-readable display name.
        #[arg(long)]
        name: Option<String>,
        /// IANA timezone name (e.g., `America/Los_Angeles`).
        #[arg(long)]
        timezone: Option<String>,
        /// Visitor salt rotation cadence (`daily`, `weekly`, `monthly`).
        #[arg(long)]
        salt_rotation: Option<String>,
        /// Drop `Sec-GPC: 1` hits (`true` or `false`).
        #[arg(long, num_args = 0..=1, default_missing_value = "true", value_parser = clap::value_parser!(bool))]
        respect_gpc: Option<bool>,
        /// Allowed browser origin (repeatable).
        #[arg(long = "origin")]
        origins: Vec<String>,
        /// Preserved query parameter key beyond `utm_*` (repeatable).
        #[arg(long = "allow-query")]
        allow_query: Vec<String>,
        /// Raw event retention window in days.
        #[arg(long)]
        retention_days: Option<u32>,
    },
    /// List all sites in the active organization or local store.
    List,
    /// Show detailed configuration for a site.
    Show {
        /// Target site identifier (`site_...` or `slug`).
        site: String,
    },
    /// Purge site analytics strictly older than `--before` (`YYYY-MM-DD` or `RFC 3339`).
    Purge {
        /// Target site identifier (`site_...` or `slug`).
        site: String,
        /// Exclusive UTC cutoff date (`YYYY-MM-DD`) or `RFC 3339` timestamp.
        #[arg(long)]
        before: String,
    },
    /// Delete a site and cascade all its analytics data.
    Delete {
        /// Target site identifier (`site_...` or `slug`).
        site: String,
        /// Required confirmation flag to prevent accidental data loss.
        #[arg(long)]
        confirm: bool,
    },
}

/// `rotameter key` subcommand tree.
#[derive(Debug, Args)]
pub struct KeyArgs {
    /// API key operation to run.
    #[command(subcommand)]
    pub action: KeyCommand,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}

/// API key management actions.
#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    /// Mint a new scoped API key (`ingest`, `read`, or `admin`).
    Create {
        /// Authorization scope (`ingest`, `read`, or `admin`).
        #[arg(long)]
        scope: String,
        /// Target site (`site_...` or `slug`), required for `ingest` and `read`.
        #[arg(long)]
        site: Option<String>,
        /// Descriptive operator label.
        #[arg(long)]
        label: String,
    },
    /// List API key metadata (`key_prefix`, scope, site, revocation status).
    List,
    /// Revoke an active API key by `key_...` UID.
    Revoke {
        /// Key identifier (`key_...`).
        key_id: String,
    },
}

/// Options for `rotameter query <site>`.
#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Target site identifier (`site_...` or `slug`).
    pub site: String,
    /// Preset range (`24h`, `7d`, `30d`, `12m`, `all`).
    #[arg(long, default_value = "30d")]
    pub range: String,
    /// Explicit inclusive start timestamp (`RFC 3339`).
    #[arg(long)]
    pub from: Option<String>,
    /// Explicit exclusive end timestamp (`RFC 3339`).
    #[arg(long)]
    pub to: Option<String>,
    /// Time-series bucket granularity (`hour`, `day`, or `month`).
    #[arg(long)]
    pub granularity: Option<String>,
    /// Metric to request (repeatable).
    #[arg(long = "metric")]
    pub metrics: Vec<String>,
    /// Breakdown dimension (repeatable).
    #[arg(long = "group-by")]
    pub group_by: Vec<String>,
    /// Filter expression (`dim=val`, `dim!=val`, `dim^=val`, `dim~=val`).
    #[arg(long = "filter")]
    pub filters: Vec<String>,
    /// Include KPI comparison against the immediately preceding window.
    #[arg(long)]
    pub compare: bool,
    /// Maximum breakdown rows (`1..=1000`).
    #[arg(long, default_value_t = 50)]
    pub limit: u32,
    /// Breakdown row pagination offset.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}

/// Options for `rotameter realtime <site>`.
#[derive(Debug, Args)]
pub struct RealtimeArgs {
    /// Target site identifier (`site_...` or `slug`).
    pub site: String,
    /// Continuously poll and refresh realtime output.
    #[arg(long)]
    pub watch: bool,
    /// Polling interval in seconds when `--watch` is enabled.
    #[arg(long, default_value_t = 5)]
    pub interval: u64,
    /// Optional maximum number of polls when `--watch` is enabled.
    #[arg(long)]
    pub max_ticks: Option<u32>,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}

/// Input log format accepted by `rotameter import`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum ImportFormat {
    /// Auto-detect `JSONL`, `caddy-json`, or `CLF` from non-empty lines.
    #[default]
    Auto,
    /// Line-delimited JSON hit objects.
    Jsonl,
    /// Nginx/Caddy/Apache Common or Combined Log Format.
    Clf,
    /// Caddy structured JSON access logs.
    #[value(name = "caddy-json")]
    CaddyJson,
}

/// Unix socket mode for `rotameter import --unix-socket`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum SocketMode {
    /// Auto-detect datagram (`syslog`) vs stream (`net unix//`) framing.
    #[default]
    Auto,
    /// Unix datagram socket (e.g. Nginx `access_log syslog:server=unix:...`).
    Dgram,
    /// Unix stream socket (e.g. Caddy `output net unix//...`).
    Stream,
}

/// Options for `rotameter import`.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)]
pub struct ImportArgs {
    /// Target site identifier (`site_...` or `slug`).
    pub site: String,
    /// Input `JSONL` or access log file path (use `-` or `--stdin` for standard input).
    pub file: Option<PathBuf>,
    /// Read log lines incrementally from `stdin` (Apache `CustomLog "|..."` pipe mode).
    #[arg(long)]
    pub stdin: bool,
    /// Listen on a Unix domain socket (`--socket-mode` selects datagram vs stream).
    #[arg(long)]
    pub unix_socket: Option<PathBuf>,
    /// Unix socket mode (`auto`, `dgram`, or `stream`).
    #[arg(long, value_enum, default_value_t = SocketMode::Auto)]
    pub socket_mode: SocketMode,
    /// Tail an appending log file continuously, handling log rotation / truncation.
    #[arg(long)]
    pub follow: bool,
    /// Input line parser (`auto`, `jsonl`, `clf`, or `caddy-json`).
    #[arg(long = "input-format", value_enum, default_value_t = ImportFormat::Auto)]
    pub input_format: ImportFormat,
    /// Parse and validate without writing or sending batches.
    #[arg(long)]
    pub dry_run: bool,
    /// Events per batch upload (`1..=500`).
    #[arg(long, default_value_t = 500)]
    pub batch_size: usize,
    /// Maximum flush wait in milliseconds during streaming (`--stdin`, `--unix-socket`, `--follow`).
    #[arg(
        long = "flush-interval",
        alias = "flush-interval-ms",
        default_value_t = 2_000
    )]
    pub flush_interval_ms: u64,
    /// Bounded in-memory queue capacity before non-blocking backpressure drops.
    #[arg(long, default_value_t = 4_096)]
    pub max_queue: usize,
    /// Disable proxy noise filtering (static assets, non-GET/HEAD, non-200/404 statuses).
    #[arg(long)]
    pub no_proxy_filter: bool,
    /// Output format override (`table`, `json`, or `csv`).
    #[command(flatten)]
    pub output: OutputFmtArg,
}
