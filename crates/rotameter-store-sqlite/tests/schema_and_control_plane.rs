#![forbid(unsafe_code)]
//! Integration tests for `SQLite` WAL initialization, migrations, and `ControlPlaneStore`.

use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::privacy::compute_api_key_hash;
use rotameter_core::storage::{ControlPlaneStore, EventIngestStore, IngestEvent, WriteBatch};
use rotameter_core::types::{ApiKeyRecord, EventKind, EventProps, KeyScope, SaltRotation, Site};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn sample_site(org_id: OrgId, site_id: SiteId, slug: &str) -> Site {
    Site {
        site_id,
        org_id,
        slug: slug.to_string(),
        display_name: "Colin Wolff".to_string(),
        timezone: "America/Los_Angeles".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: vec!["https://colinwolff.dev".to_string()],
        allowed_query_keys: vec!["range".to_string()],
        raw_retention_days: Some(365),
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_000,
    }
}

#[test]
fn opens_wal_db_verifies_pragmas_and_runs_site_crud() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("nested").join("rotameter.db");

    let store = SqliteStore::open(&db_path, SqliteConfig::default(), 1_700_000_000_000).unwrap();
    let health = store.health_snapshot().unwrap();
    assert_eq!(health.journal_mode, "wal");
    assert_eq!(health.schema_version, 1);
    assert_eq!(health.cache_size, -16_000);
    assert_eq!(health.mmap_size, 67_108_864);
    assert!(health.integrity_ok);

    let org = store.ensure_default_org(1_700_000_000_000).unwrap();
    assert_eq!(org.slug, "default");
    assert_eq!(
        store.ensure_default_org(1_700_000_005_000).unwrap().org_id,
        org.org_id
    );

    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]);
    let mut site = sample_site(org.org_id, site_id, "colinwolff.dev");
    store.upsert_site(&site).unwrap();

    site.slug = "blog.colinwolff.dev".to_string();
    site.display_name = "Blog".to_string();
    site.updated_at_ms = 1_700_000_010_000;
    assert_eq!(store.upsert_site(&site).unwrap().site_id, site_id);

    assert_eq!(
        store
            .resolve_site(org.org_id, &SiteRef::Id(site_id))
            .unwrap()
            .unwrap()
            .display_name,
        "Blog"
    );
    assert_eq!(
        store
            .resolve_site(
                org.org_id,
                &SiteRef::Slug("blog.colinwolff.dev".to_string())
            )
            .unwrap()
            .unwrap()
            .site_id,
        site_id
    );
    assert_eq!(store.list_sites(org.org_id).unwrap().len(), 1);

    drop(store);
    let reopened = SqliteStore::open(&db_path, SqliteConfig::default(), 1_700_000_030_000).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 1);
}

#[test]
fn api_keys_and_site_deletion_cascade_cleanly() {
    let store = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000).unwrap();
    let org = store.ensure_default_org(1_700_000_000_000).unwrap();
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]);
    store
        .upsert_site(&sample_site(org.org_id, site_id, "colinwolff.dev"))
        .unwrap();

    let hash = compute_api_key_hash("rm_rd_secret_token");
    let key = ApiKeyRecord {
        key_id: KeyId::from_uuidv7_parts(1_700_000_000_000, [2; 10]),
        org_id: org.org_id,
        key_prefix: "rm_rd_secret".to_string(),
        key_hash: hash,
        scope: KeyScope::Read,
        site_id: Some(site_id),
        label: "cli".to_string(),
        created_at_ms: 1_700_000_000_000,
        last_used_ms: None,
        revoked_at_ms: None,
    };
    store.create_api_key(&key).unwrap();
    assert_eq!(
        store
            .find_active_key_by_hash(&hash)
            .unwrap()
            .unwrap()
            .key_id,
        key.key_id
    );
    assert!(store.revoke_api_key(key.key_id, 1_700_000_020_000).unwrap());
    assert!(store.find_active_key_by_hash(&hash).unwrap().is_none());
    assert_eq!(store.list_api_keys(org.org_id).unwrap().len(), 1);

    store
        .write_batch(&WriteBatch {
            events: vec![IngestEvent {
                site_id,
                ts_ms: 1_700_000_000_000,
                session_id: [3; 16],
                seq_in_session: 1,
                visitor_id: [4; 16],
                event_kind: EventKind::Pageview,
                path: "/".to_string(),
                props: EventProps::default(),
            }],
            sessions: Vec::new(),
            rollups: Vec::new(),
        })
        .unwrap();
    assert!(store.delete_site(site_id).unwrap());
    assert_eq!(store.list_sites(org.org_id).unwrap().len(), 0);
    assert_eq!(store.list_api_keys(org.org_id).unwrap().len(), 0);
}
