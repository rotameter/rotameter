#![forbid(unsafe_code)]
//! Synchronous sessionization, cardinality protection, and [`WriteBatch`] assembly.

use std::collections::HashMap;

use rotameter_core::cardinality::{CardinalityGuard, OVERFLOW_BUCKET};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{SessionId, utc_hour_floor_ms, utc_yyyymmdd};
use rotameter_core::session::{DEFAULT_SESSION_TIMEOUT_MS, SessionTracker};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{DimensionKind, EventKind, UtmParams};

use crate::ingest::types::{QueuedEvent, lookup_fallback};
use crate::state::{ServerStore, new_session_id};

pub(crate) struct IngestProcessor {
    sessions: SessionTracker,
    cardinality: CardinalityGuard,
}

impl IngestProcessor {
    pub(crate) fn new() -> Self {
        Self {
            sessions: SessionTracker::with_defaults(),
            cardinality: CardinalityGuard::with_defaults(),
        }
    }

    pub(crate) fn process_batch(
        &mut self,
        store: &dyn ServerStore,
        batch: Vec<QueuedEvent>,
    ) -> Result<(), CoreError> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut write = WriteBatch::default();
        let mut session_map: HashMap<(SiteId, SessionId), SessionUpsert> = HashMap::new();
        let mut rollup_map: HashMap<(SiteId, i64), RollupDelta> = HashMap::new();

        for ev in batch {
            self.fold_event(
                store,
                ev,
                &mut write.events,
                &mut session_map,
                &mut rollup_map,
            )?;
        }

        write.sessions = session_map.into_values().collect();
        write.rollups = rollup_map.into_values().collect();
        store.write_batch(&write)
    }

    fn fold_event(
        &mut self,
        store: &dyn ServerStore,
        ev: QueuedEvent,
        events_out: &mut Vec<IngestEvent>,
        session_map: &mut HashMap<(SiteId, SessionId), SessionUpsert>,
        rollup_map: &mut HashMap<(SiteId, i64), RollupDelta>,
    ) -> Result<(), CoreError> {
        let min_last_ms = ev.ts_ms.saturating_sub(DEFAULT_SESSION_TIMEOUT_MS);
        let fallback = lookup_fallback(store, ev.site_id, ev.candidates, min_last_ms)?;
        let is_pv = matches!(ev.event_kind, EventKind::Pageview);
        let delta = self.sessions.record_event(
            ev.site_id,
            ev.candidates,
            ev.ts_ms,
            is_pv,
            fallback,
            new_session_id,
        );

        let path = self.cap(ev.site_id, DimensionKind::Path, ev.ts_ms, &ev.path);
        let event_kind = self.cap_event_kind(ev.site_id, ev.ts_ms, ev.event_kind);
        let ref_host = self.cap(ev.site_id, DimensionKind::RefHost, ev.ts_ms, &ev.ref_host);
        let country = self.cap(ev.site_id, DimensionKind::GeoCountry, ev.ts_ms, &ev.country);
        let region = self.cap(ev.site_id, DimensionKind::GeoRegion, ev.ts_ms, &ev.region);
        let browser = self.cap(ev.site_id, DimensionKind::UaBrowser, ev.ts_ms, &ev.browser);
        let os = self.cap(ev.site_id, DimensionKind::UaOs, ev.ts_ms, &ev.os);
        let utm = self.cap_utm(ev.site_id, ev.ts_ms, ev.utm);

        events_out.push(IngestEvent {
            site_id: ev.site_id,
            ts_ms: ev.ts_ms,
            session_id: delta.session_id,
            seq_in_session: delta.seq_in_session,
            visitor_id: delta.visitor_id,
            event_kind,
            path: path.clone(),
            props: ev.props,
        });

        session_map
            .entry((ev.site_id, delta.session_id))
            .and_modify(|existing| {
                existing.last_event_ms = existing.last_event_ms.max(delta.last_event_ms);
                existing.event_count = existing.event_count.max(delta.event_count);
                existing.pageview_count = existing.pageview_count.max(delta.pageview_count);
                existing.exit_path.clone_from(&path);
            })
            .or_insert_with(|| SessionUpsert {
                site_id: ev.site_id,
                session_id: delta.session_id,
                visitor_id: delta.visitor_id,
                started_at_ms: delta.started_at_ms,
                last_event_ms: delta.last_event_ms,
                event_count: delta.event_count,
                pageview_count: delta.pageview_count,
                entry_path: path.clone(),
                exit_path: path,
                ref_host,
                channel: ev.channel,
                utm,
                country,
                region,
                browser,
                os,
                device_type: ev.device_type,
                viewport_class: ev.viewport_class,
            });

        let hour_bucket = utc_hour_floor_ms(delta.started_at_ms);
        let r_entry = rollup_map
            .entry((ev.site_id, hour_bucket))
            .or_insert_with(|| RollupDelta {
                site_id: ev.site_id,
                hour_bucket_ms: hour_bucket,
                day_utc: utc_yyyymmdd(delta.started_at_ms),
                pageviews: 0,
                events_total: 0,
                bounced_visits: 0,
                visits_started: 0,
                total_duration_ms: 0,
            });
        r_entry.pageviews += i64::from(is_pv);
        r_entry.events_total += 1;
        r_entry.bounced_visits += i64::from(delta.bounce_delta);
        r_entry.visits_started += i64::from(delta.is_new_session);
        r_entry.total_duration_ms += delta.duration_delta_ms;
        Ok(())
    }

    fn cap(&mut self, site_id: SiteId, kind: DimensionKind, ts_ms: i64, val: &str) -> String {
        self.cardinality
            .check_or_record(site_id, kind, ts_ms, val)
            .as_str()
            .to_string()
    }

    fn cap_event_kind(&mut self, site_id: SiteId, ts_ms: i64, kind: EventKind) -> EventKind {
        match kind {
            EventKind::Custom(name) => {
                if self
                    .cardinality
                    .check_or_record(site_id, DimensionKind::EventName, ts_ms, &name)
                    .is_overflowed()
                {
                    EventKind::Custom(OVERFLOW_BUCKET.to_string())
                } else {
                    EventKind::Custom(name)
                }
            }
            built_in => built_in,
        }
    }

    fn cap_utm(&mut self, site_id: SiteId, ts_ms: i64, utm: UtmParams) -> UtmParams {
        let cap_opt = |proc: &mut Self, opt: Option<String>| {
            opt.map(|s| proc.cap(site_id, DimensionKind::Utm, ts_ms, &s))
        };
        UtmParams {
            source: cap_opt(self, utm.source),
            medium: cap_opt(self, utm.medium),
            campaign: cap_opt(self, utm.campaign),
            content: cap_opt(self, utm.content),
            term: cap_opt(self, utm.term),
        }
    }
}
