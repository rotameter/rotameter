#![forbid(unsafe_code)]
//! Integration tests for batched writes, dimension interning, session fallback, and backups.

use rotameter_core::id::{OrgId, SiteId};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::backup::verify_integrity;
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use rusqlite::Connection;

fn seed_site(store: &SqliteStore, org_id: OrgId, site_id: SiteId, slug: &str) {
    assert!(
        store
            .upsert_site_sync(&Site {
                site_id,
                org_id,
                slug: slug.to_string(),
                display_name: slug.to_string(),
                timezone: "UTC".to_string(),
                salt_rotation: SaltRotation::Daily,
                respect_gpc: false,
                is_self_site: false,
                allowed_origins: Vec::new(),
                allowed_query_keys: Vec::new(),
                raw_retention_days: None,
                created_at_ms: 1_700_000_000_000,
                updated_at_ms: 1_700_000_000_000,
            })
            .is_ok()
    );
}

fn sample_batch(site_a: SiteId, site_b: SiteId, props: EventProps) -> WriteBatch {
    WriteBatch {
        events: vec![
            IngestEvent {
                site_id: site_a,
                ts_ms: 1_700_000_000_000,
                session_id: [0xaa; 16],
                seq_in_session: 1,
                visitor_id: [0x11; 16],
                event_kind: EventKind::Pageview,
                path: "/projects/rotameter".to_string(),
                props: EventProps::default(),
            },
            IngestEvent {
                site_id: site_a,
                ts_ms: 1_700_000_060_000,
                session_id: [0xaa; 16],
                seq_in_session: 2,
                visitor_id: [0x11; 16],
                event_kind: EventKind::Custom("signup_clicked".to_string()),
                path: "/projects/rotameter".to_string(),
                props,
            },
            IngestEvent {
                site_id: site_b,
                ts_ms: 1_700_000_010_000,
                session_id: [0xbb; 16],
                seq_in_session: 1,
                visitor_id: [0x22; 16],
                event_kind: EventKind::Pageview,
                path: "/projects/rotameter".to_string(),
                props: EventProps::default(),
            },
        ],
        sessions: vec![SessionUpsert {
            site_id: site_a,
            session_id: [0xaa; 16],
            visitor_id: [0x11; 16],
            started_at_ms: 1_700_000_000_000,
            last_event_ms: 1_700_000_060_000,
            event_count: 2,
            pageview_count: 1,
            entry_path: "/projects/rotameter".to_string(),
            exit_path: "/projects/rotameter".to_string(),
            ref_host: "(direct)".to_string(),
            channel: Channel::Direct,
            utm: UtmParams::default(),
            country: "US".to_string(),
            region: "CA".to_string(),
            browser: "Chrome 131".to_string(),
            os: "macOS 15".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Xl,
        }],
        rollups: vec![
            RollupDelta {
                site_id: site_a,
                hour_bucket_ms: 1_699_999_200_000,
                day_utc: 20_231_114,
                pageviews: 1,
                events_total: 1,
                bounced_visits: 1,
                visits_started: 1,
                total_duration_ms: 0,
            },
            RollupDelta {
                site_id: site_a,
                hour_bucket_ms: 1_699_999_200_000,
                day_utc: 20_231_114,
                pageviews: 0,
                events_total: 1,
                bounced_visits: -1,
                visits_started: 0,
                total_duration_ms: 60_000,
            },
        ],
    }
}

#[test]
fn batched_writer_interns_dimensions_updates_rollups_and_vacuums_backup() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("rotameter.db");
    let backup_path = dir.path().join("backups").join("snapshot.db");

    let store = SqliteStore::open(&db_path, SqliteConfig::default(), 1_700_000_000_000).unwrap();
    let org = store.ensure_default_org_sync(1_700_000_000_000).unwrap();
    let site_a = SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]);
    let site_b = SiteId::from_uuidv7_parts(1_700_000_000_000, [2; 10]);
    seed_site(&store, org.org_id, site_a, "site-a.dev");
    seed_site(&store, org.org_id, site_b, "site-b.dev");

    let props = EventProps::from_pairs([("plan", "pro")]).unwrap();
    store
        .write_batch_sync(&sample_batch(site_a, site_b, props))
        .unwrap();

    let active = store
        .lookup_active_session_sync(site_a, [0x11; 16], 1_699_999_000_000)
        .unwrap()
        .unwrap();
    assert_eq!(active.session_id, [0xaa; 16]);
    assert_eq!(active.event_count, 2);
    assert_eq!(active.last_seq, 2);

    store.checkpoint_truncate().unwrap();
    store.vacuum_into(&backup_path).unwrap();
    assert!(store.vacuum_into(&backup_path).is_err());

    let restored = Connection::open(&backup_path).unwrap();
    assert!(verify_integrity(&restored).unwrap());
    let events_count: i64 = restored
        .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
        .unwrap();
    let bounced: i64 = restored
        .query_row("SELECT bounced_visits FROM rollups_daily", [], |r| r.get(0))
        .unwrap();
    let duration: i64 = restored
        .query_row("SELECT total_duration_ms FROM rollups_hourly", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(events_count, 3);
    assert_eq!(bounced, 0);
    assert_eq!(duration, 60_000);
}
