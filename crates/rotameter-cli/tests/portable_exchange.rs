#![forbid(unsafe_code)]
//! Integration tests for feature-gated `rotameter backup --format portable`, `restore`, and `migrate`.
#![cfg(all(feature = "sqlite", feature = "arrow"))]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rotameter_cli::run;
use rotameter_core::id::{KeyId, SiteId, SiteRef};
use rotameter_core::privacy::utc_yyyymmdd;
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    ApiKeyRecord, Channel, DeviceType, EventKind, EventProps, KeyScope, SaltRotation, Site,
    UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn seed_source_db(path: &Path, ts: i64) -> Result<(Site, QuerySpec), String> {
    let store = SqliteStore::open(path, SqliteConfig::default(), ts).map_err(|e| e.to_string())?;
    let org = store
        .ensure_default_org_sync(ts)
        .map_err(|e| e.to_string())?;
    let site_id = SiteId::from_uuidv7_parts(u64::try_from(ts).unwrap_or(0), [4; 10]);
    let site = Site {
        site_id,
        org_id: org.org_id,
        slug: "colinwolff.dev".to_string(),
        display_name: "Colin Wolff".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: vec!["https://colinwolff.dev".to_string()],
        allowed_query_keys: vec!["tab".to_string()],
        raw_retention_days: Some(365),
        created_at_ms: ts,
        updated_at_ms: ts,
    };
    store.upsert_site_sync(&site).map_err(|e| e.to_string())?;
    store
        .create_api_key_sync(&ApiKeyRecord {
            key_id: KeyId::from_uuidv7_parts(u64::try_from(ts).unwrap_or(0), [8; 10]),
            org_id: org.org_id,
            key_prefix: "rm_rd_test".to_string(),
            key_hash: [0x11; 32],
            scope: KeyScope::Read,
            site_id: Some(site_id),
            label: "reader".to_string(),
            created_at_ms: ts,
            last_used_ms: None,
            revoked_at_ms: None,
        })
        .map_err(|e| e.to_string())?;
    let hour_bucket_ms = ts - ts.rem_euclid(3_600_000);
    store
        .write_batch_sync(&WriteBatch {
            events: vec![IngestEvent {
                site_id,
                ts_ms: ts,
                session_id: [1; 16],
                seq_in_session: 1,
                visitor_id: [2; 16],
                event_kind: EventKind::Pageview,
                path: "/pricing".to_string(),
                props: EventProps::default(),
            }],
            sessions: vec![SessionUpsert {
                site_id,
                session_id: [1; 16],
                visitor_id: [2; 16],
                started_at_ms: ts,
                last_event_ms: ts,
                event_count: 1,
                pageview_count: 1,
                entry_path: "/pricing".to_string(),
                exit_path: "/pricing".to_string(),
                ref_host: "(direct)".to_string(),
                channel: Channel::Direct,
                utm: UtmParams::default(),
                country: "US".to_string(),
                region: "CA".to_string(),
                browser: "Firefox 130".to_string(),
                os: "macOS 15".to_string(),
                device_type: DeviceType::Desktop,
                viewport_class: ViewportClass::Xl,
            }],
            rollups: vec![RollupDelta {
                site_id,
                hour_bucket_ms,
                day_utc: utc_yyyymmdd(ts),
                pageviews: 1,
                events_total: 1,
                bounced_visits: 1,
                visits_started: 1,
                total_duration_ms: 0,
            }],
        })
        .map_err(|e| e.to_string())?;
    let spec = QuerySpec {
        site_id: SiteRef::Slug("colinwolff.dev".to_string()),
        range: TimeRange::Preset {
            preset: RangePreset::All,
        },
        granularity: None,
        metrics: vec![Metric::Visitors, Metric::Pageviews],
        group_by: vec![],
        filters: vec![],
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::default(),
    };
    Ok((site, spec))
}

fn assert_parity(
    src_db: &Path,
    destinations: &[PathBuf],
    site: &Site,
    spec: &QuerySpec,
    ts: i64,
) -> Result<(), String> {
    let src = SqliteStore::open(src_db, SqliteConfig::default(), ts).map_err(|e| e.to_string())?;
    let expected = src
        .query_sync(site, spec, ts + 5_000)
        .map_err(|e| e.to_string())?;
    for db in destinations {
        let s = SqliteStore::open(db, SqliteConfig::default(), ts + 5_000)
            .map_err(|e| e.to_string())?;
        let org = s
            .ensure_default_org_sync(ts + 5_000)
            .map_err(|e| e.to_string())?;
        if s.list_sites_sync(org.org_id)
            .map_err(|e| e.to_string())?
            .len()
            != 1
            || s.list_api_keys_sync(org.org_id)
                .map_err(|e| e.to_string())?
                .len()
                != 1
        {
            return Err("control-plane count mismatch".to_string());
        }
        let actual = s
            .query_sync(site, spec, ts + 5_000)
            .map_err(|e| e.to_string())?;
        if actual.summary != expected.summary {
            return Err("query summary mismatch".to_string());
        }
    }
    Ok(())
}

#[test]
fn portable_directory_and_tar_zst_roundtrip() {
    let ts = 1_700_000_000_000_i64;
    let dir = tempfile::tempdir().unwrap();
    let src_db = dir.path().join("src.db");
    let (site, spec) = seed_source_db(&src_db, ts).unwrap();

    let bundle_dir = dir.path().join("bundle");
    let dst_dir_db = dir.path().join("restored_dir.db");
    let src_s = src_db.to_str().unwrap();
    let b_s = bundle_dir.to_str().unwrap();
    assert_eq!(
        run([
            "rotameter",
            "--db",
            src_s,
            "backup",
            "--format",
            "portable",
            b_s
        ]),
        ExitCode::SUCCESS
    );
    assert!(bundle_dir.join("manifest.json").exists());
    assert!(bundle_dir.join("control_plane.json").exists());
    assert!(bundle_dir.join("events/day=2023-11-14.parquet").exists());
    let uri = format!("sqlite://{}", dst_dir_db.display());
    assert_eq!(
        run(["rotameter", "restore", b_s, "--to", &uri]),
        ExitCode::SUCCESS
    );

    let archive = dir.path().join("snapshot.tar.zst");
    let dst_tar_db = dir.path().join("restored_tar.db");
    let arc_s = archive.to_str().unwrap();
    assert_eq!(
        run([
            "rotameter",
            "--db",
            src_s,
            "backup",
            "--format",
            "portable",
            arc_s
        ]),
        ExitCode::SUCCESS
    );
    assert_eq!(
        run([
            "rotameter",
            "restore",
            arc_s,
            "--to",
            dst_tar_db.to_str().unwrap()
        ]),
        ExitCode::SUCCESS
    );
    assert_parity(&src_db, &[dst_dir_db, dst_tar_db], &site, &spec, ts).unwrap();

    fs::write(bundle_dir.join("control_plane.json"), b"{}").unwrap();
    let tampered = dir.path().join("tampered.db");
    assert_ne!(
        run([
            "rotameter",
            "restore",
            b_s,
            "--to",
            tampered.to_str().unwrap()
        ]),
        ExitCode::SUCCESS
    );
}
