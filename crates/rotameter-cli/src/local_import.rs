#![forbid(unsafe_code)]
//! Direct `SQLite` ingestion path for `rotameter import --db ...`.

use std::collections::HashMap;

use rotameter_client::types::{BatchEventItem, SiteRef};
use rotameter_core::cardinality::{CardinalityGuard, OVERFLOW_BUCKET};
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{InMemorySaltProvider, SessionId, resolve_visitor_candidates};
use rotameter_core::session::{DEFAULT_SESSION_TIMEOUT_MS, SessionTracker};
use rotameter_core::storage::{
    ControlPlaneStore, EventIngestStore, IngestEvent, RollupDelta, SessionUpsert, WriteBatch,
};
use rotameter_core::types::{DimensionKind, EventKind, Site};
use rotameter_store_sqlite::SqliteStore;

use crate::cli::GlobalOpts;
use crate::context::open_local_store;
use crate::exit::CliError;
use crate::local_import_norm::{normalize_local_item, record_session_and_rollup};

pub(crate) struct LocalFoldState<'a> {
    pub(crate) store: &'a SqliteStore,
    pub(crate) site: &'a Site,
    pub(crate) salts: &'a InMemorySaltProvider,
    pub(crate) sessions: SessionTracker,
    pub(crate) cardinality: CardinalityGuard,
    pub(crate) write: WriteBatch,
    pub(crate) session_map: HashMap<(SiteId, SessionId), SessionUpsert>,
    pub(crate) rollup_map: HashMap<(SiteId, i64), RollupDelta>,
}

pub(crate) fn ingest_local(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    items: &[BatchEventItem],
) -> Result<(u64, u64), CliError> {
    let (store, _, org_id) = open_local_store(global)?;
    let site = store
        .resolve_site(org_id, site_ref)?
        .ok_or_else(|| CliError::runtime(format!("unknown site `{site_ref}`")))?;
    let salts = InMemorySaltProvider::new(|_| {
        let mut seed = [0x42_u8; 32];
        let _ = getrandom::fill(&mut seed);
        seed
    });
    let mut ctx = LocalFoldState {
        store: &store,
        site: &site,
        salts: &salts,
        sessions: SessionTracker::with_defaults(),
        cardinality: CardinalityGuard::with_defaults(),
        write: WriteBatch::default(),
        session_map: HashMap::new(),
        rollup_map: HashMap::new(),
    };

    let (mut accepted, mut rejected) = (0_u64, 0_u64);
    for item in items {
        if fold_local_item(&mut ctx, item)? {
            accepted += 1;
        } else {
            rejected += 1;
        }
    }
    ctx.write.sessions = ctx.session_map.into_values().collect();
    ctx.write.rollups = ctx.rollup_map.into_values().collect();
    store.write_batch(&ctx.write)?;
    Ok((accepted, rejected))
}

fn fold_local_item(ctx: &mut LocalFoldState<'_>, item: &BatchEventItem) -> Result<bool, CliError> {
    let Some((ts_ms, event_kind, norm_url, norm_ref, props)) = normalize_local_item(ctx.site, item)
    else {
        return Ok(false);
    };
    let hint = item.visitor_hint.as_deref().unwrap_or("import");
    let candidates = resolve_visitor_candidates(
        ctx.salts,
        ctx.site.site_id,
        ctx.site.salt_rotation,
        ts_ms,
        hint,
        "",
    )?;
    let fallback = ctx.store.lookup_active_session(
        ctx.site.site_id,
        candidates.current,
        ts_ms.saturating_sub(DEFAULT_SESSION_TIMEOUT_MS),
    )?;
    let is_pv = matches!(event_kind, EventKind::Pageview);
    let mut sid = [0_u8; 16];
    let _ = getrandom::fill(&mut sid);
    let delta =
        ctx.sessions
            .record_event(ctx.site.site_id, candidates, ts_ms, is_pv, fallback, || sid);

    let site_id = ctx.site.site_id;
    let path = ctx
        .cardinality
        .check_or_record(site_id, DimensionKind::Path, ts_ms, &norm_url.path)
        .as_str()
        .to_string();
    let ref_host = ctx
        .cardinality
        .check_or_record(site_id, DimensionKind::RefHost, ts_ms, &norm_ref.source)
        .as_str()
        .to_string();
    let ek = match event_kind {
        EventKind::Custom(name)
            if ctx
                .cardinality
                .check_or_record(site_id, DimensionKind::EventName, ts_ms, &name)
                .is_overflowed() =>
        {
            EventKind::Custom(OVERFLOW_BUCKET.to_string())
        }
        other => other,
    };
    ctx.write.events.push(IngestEvent {
        site_id,
        ts_ms,
        session_id: delta.session_id,
        seq_in_session: delta.seq_in_session,
        visitor_id: delta.visitor_id,
        event_kind: ek,
        path: path.clone(),
        props,
    });
    record_session_and_rollup(
        ctx,
        &delta,
        path,
        ref_host,
        norm_ref.channel,
        item.viewport_width,
        is_pv,
    );
    Ok(true)
}
