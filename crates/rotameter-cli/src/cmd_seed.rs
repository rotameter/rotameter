#![forbid(unsafe_code)]
//! Handler for `rotameter seed` synthetic analytics generator.

use clap::Args;
#[cfg(not(feature = "sqlite"))]
use rotameter_core::id::validate_slug;
#[cfg(feature = "sqlite")]
use rotameter_core::id::{SiteId, SiteRef, validate_slug};
#[cfg(feature = "sqlite")]
use rotameter_core::privacy::MS_PER_DAY;
#[cfg(feature = "sqlite")]
use rotameter_core::types::{SaltRotation, Site};
#[cfg(feature = "sqlite")]
use serde::Serialize;

use crate::cli::GlobalOpts;
#[cfg(feature = "sqlite")]
use crate::context::{open_local_store, print_formatted, resolve_db_path};
use crate::exit::CliError;
#[cfg(feature = "sqlite")]
use crate::local_keys::random_bytes;
#[cfg(feature = "sqlite")]
use crate::seed_gen;

/// Options for `rotameter seed`.
#[derive(Debug, Clone, Args)]
pub struct SeedArgs {
    /// Target site slug to create or populate.
    #[arg(long, default_value = "demo.local")]
    pub site: String,
    /// Display name when creating or updating the target site.
    #[arg(long)]
    pub display_name: Option<String>,
    /// Number of trailing days of historical traffic to synthesize (`1..=365`).
    #[arg(long, default_value_t = 30)]
    pub days: u32,
    /// Baseline average sessions generated per day (`1..=50000`).
    #[arg(long, default_value_t = 240)]
    pub sessions_per_day: u32,
    /// Distinct active visitors placed inside the trailing 5-minute realtime window (`0..=5000`).
    #[arg(long, default_value_t = 18)]
    pub realtime_visitors: u32,
    /// Deterministic PRNG seed controlling traffic curves and breakdowns.
    #[arg(long, default_value_t = 20_261_122)]
    pub seed: u64,
    /// Purge existing analytics data for the target site before seeding.
    #[arg(long)]
    pub reset: bool,
}

#[cfg(feature = "sqlite")]
#[derive(Debug, Serialize)]
struct SeedSummary {
    site: String,
    site_id: String,
    days: u32,
    sessions: u64,
    events: u64,
    realtime_visitors: u64,
    seed: u64,
    db: String,
}

pub(crate) fn run_seed(global: &GlobalOpts, args: &SeedArgs) -> Result<(), CliError> {
    validate_args(args)?;
    run_seed_impl(global, args)
}

fn validate_args(args: &SeedArgs) -> Result<String, CliError> {
    let slug = validate_slug(&args.site)?;
    if !(1..=365).contains(&args.days) {
        return Err(CliError::usage("`--days` must be between 1 and 365"));
    }
    if !(1..=50_000).contains(&args.sessions_per_day) {
        return Err(CliError::usage(
            "`--sessions-per-day` must be between 1 and 50000",
        ));
    }
    if args.realtime_visitors > 5_000 {
        return Err(CliError::usage(
            "`--realtime-visitors` must be at most 5000",
        ));
    }
    Ok(slug)
}

#[cfg(feature = "sqlite")]
fn run_seed_impl(global: &GlobalOpts, args: &SeedArgs) -> Result<(), CliError> {
    let slug = validate_slug(&args.site)?;
    let db = resolve_db_path(global, &|k| std::env::var(k).ok(), None);
    let (store, now, org_id) = open_local_store(global)?;

    let existing = store.resolve_site_sync(org_id, &SiteRef::Slug(slug.clone()))?;
    let site = if let Some(mut prev) = existing {
        if let Some(name) = &args.display_name {
            prev.display_name.clone_from(name);
            prev.updated_at_ms = now;
            store.upsert_site_sync(&prev)?
        } else {
            prev
        }
    } else {
        let site_id = SiteId::from_uuidv7_parts(
            u64::try_from(now.max(0)).unwrap_or(0),
            random_bytes::<10>()?,
        );
        let new_site = Site {
            site_id,
            org_id,
            slug: slug.clone(),
            display_name: args
                .display_name
                .clone()
                .unwrap_or_else(|| "Demo (Seeded)".to_string()),
            timezone: "UTC".to_string(),
            salt_rotation: SaltRotation::Daily,
            respect_gpc: false,
            is_self_site: false,
            allowed_origins: Vec::new(),
            allowed_query_keys: Vec::new(),
            raw_retention_days: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        store.upsert_site_sync(&new_site)?
    };

    if args.reset {
        store.purge_before_sync(site.site_id, now.saturating_add(MS_PER_DAY))?;
    }

    let stats = seed_gen::generate_and_write(&store, &site, now, args)?;
    let summary = SeedSummary {
        site: site.slug,
        site_id: site.site_id.to_string(),
        days: args.days,
        sessions: stats.sessions,
        events: stats.events,
        realtime_visitors: stats.realtime_visitors,
        seed: args.seed,
        db,
    };
    let row = vec![
        summary.site.clone(),
        summary.site_id.clone(),
        summary.days.to_string(),
        summary.sessions.to_string(),
        summary.events.to_string(),
        summary.realtime_visitors.to_string(),
        summary.seed.to_string(),
        summary.db.clone(),
    ];
    print_formatted(
        global.format,
        &[
            "SITE",
            "SITE_ID",
            "DAYS",
            "SESSIONS",
            "EVENTS",
            "REALTIME_VISITORS",
            "SEED",
            "DB",
        ],
        &[row],
        &summary,
        global.quiet,
    )
}

#[cfg(not(feature = "sqlite"))]
fn run_seed_impl(_global: &GlobalOpts, _args: &SeedArgs) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter seed` requires the `sqlite` cargo feature",
    ))
}
