#![forbid(unsafe_code)]
//! Integration tests for `eq`/`neq`/`in`/`contains`/`prefix` SQL filters and CSV export.

use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{
    Dimension, FilterOp, FilterValue, Metric, QueryFilter, QueryFormat, QuerySpec, TimeRange,
};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore, format_query_result_csv};

fn seed(ts_ms: i64) -> (tempfile::TempDir, SqliteStore, Site) {
    let Ok(dir) = tempfile::tempdir() else {
        std::process::abort()
    };
    let db_path = dir.path().join("rotameter.db");
    let Ok(store) = SqliteStore::open(&db_path, SqliteConfig::default(), ts_ms) else {
        std::process::abort()
    };
    let Ok(org) = store.ensure_default_org_sync(ts_ms) else {
        std::process::abort()
    };
    let site = Site {
        site_id: SiteId::from_uuidv7_parts(u64::try_from(ts_ms.max(0)).unwrap_or(0), [4; 10]),
        org_id: OrgId::from_bytes(*org.org_id.as_bytes()),
        slug: "shop".to_string(),
        display_name: "Shop".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: ts_ms,
        updated_at_ms: ts_ms,
    };
    let _ = store.upsert_site_sync(&site);

    let items = [
        (
            1_u8,
            "/promo/100%_off",
            "US",
            DeviceType::Mobile,
            ViewportClass::Xs,
        ),
        (
            2_u8,
            "/promo/100Xoff",
            "CA",
            DeviceType::Desktop,
            ViewportClass::Xl,
        ),
        (
            3_u8,
            "/docs,v1/intro",
            "DE",
            DeviceType::Tablet,
            ViewportClass::Md,
        ),
    ];
    let mut batch = WriteBatch::default();
    for (idx, path, country, device, vp) in items {
        batch.events.push(IngestEvent {
            site_id: site.site_id,
            ts_ms: ts_ms + i64::from(idx) * 1_000,
            session_id: [idx; 16],
            seq_in_session: 1,
            visitor_id: [idx; 16],
            event_kind: EventKind::Pageview,
            path: path.to_string(),
            props: EventProps::default(),
        });
        batch.sessions.push(SessionUpsert {
            site_id: site.site_id,
            session_id: [idx; 16],
            visitor_id: [idx; 16],
            started_at_ms: ts_ms + i64::from(idx) * 1_000,
            last_event_ms: ts_ms + i64::from(idx) * 1_000,
            event_count: 1,
            pageview_count: 1,
            entry_path: path.to_string(),
            exit_path: path.to_string(),
            ref_host: "(direct)".to_string(),
            channel: Channel::Direct,
            utm: UtmParams {
                source: Some("newsletter".to_string()),
                campaign: Some("spring".to_string()),
                ..UtmParams::default()
            },
            country: country.to_string(),
            region: "unknown".to_string(),
            browser: "Chrome".to_string(),
            os: "Linux".to_string(),
            device_type: device,
            viewport_class: vp,
        });
        batch.rollups.push(RollupDelta {
            site_id: site.site_id,
            hour_bucket_ms: ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR,
            day_utc: utc_yyyymmdd(ts_ms),
            pageviews: 1,
            events_total: 1,
            bounced_visits: 1,
            visits_started: 1,
            total_duration_ms: 0,
        });
    }
    let _ = store.write_batch_sync(&batch);
    (dir, store, site)
}

#[test]
fn filters_escape_like_wildcards_and_support_all_operators() {
    let t0 = 1_705_300_000_000_i64;
    let (_dir, store, site) = seed(t0);

    let wildcard_spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Explicit {
            from_ms: t0,
            to_ms: t0 + 10_000,
        },
        granularity: None,
        metrics: vec![Metric::Visits],
        group_by: vec![Dimension::Path],
        filters: vec![QueryFilter {
            dimension: Dimension::Path,
            op: FilterOp::Contains,
            value: FilterValue::Scalar("100%_".to_string()),
        }],
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::Json,
    };
    let wildcard_res = store
        .query_sync(&site, &wildcard_spec, t0 + 10_000)
        .unwrap();
    assert_eq!(wildcard_res.summary.visits, 1);
    assert_eq!(
        wildcard_res.rows[0].key.get("path").map(String::as_str),
        Some("/promo/100%_off")
    );

    let multi_spec = QuerySpec {
        filters: vec![
            QueryFilter {
                dimension: Dimension::Country,
                op: FilterOp::In,
                value: FilterValue::List(vec!["CA".to_string(), "DE".to_string()]),
            },
            QueryFilter {
                dimension: Dimension::Device,
                op: FilterOp::Neq,
                value: FilterValue::Scalar("desktop".to_string()),
            },
            QueryFilter {
                dimension: Dimension::Viewport,
                op: FilterOp::Eq,
                value: FilterValue::Scalar("md".to_string()),
            },
            QueryFilter {
                dimension: Dimension::Path,
                op: FilterOp::Prefix,
                value: FilterValue::Scalar("/docs".to_string()),
            },
        ],
        group_by: vec![Dimension::Path, Dimension::Device],
        format: QueryFormat::Csv,
        ..wildcard_spec
    };
    let res = store.query_sync(&site, &multi_spec, t0 + 10_000).unwrap();
    assert_eq!(res.summary.visits, 1);
    let csv = format_query_result_csv(&res, &multi_spec);
    assert!(csv.starts_with("path,device,visits\n"));
    assert!(csv.contains("\"/docs,v1/intro\",tablet,1\n"));
}
