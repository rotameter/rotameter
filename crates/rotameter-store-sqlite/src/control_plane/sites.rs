#![forbid(unsafe_code)]
//! `SQLite` persistence helpers for organizations (`orgs`) and sites (`sites`).

use rotameter_core::error::CoreError;
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::types::{Org, Site};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::config::map_sql_err;
use crate::intern::DimInterner;
use crate::row_decode::{decode_org, decode_site};

const SITE_SELECT: &str = "SELECT site_id, org_id, slug, display_name, timezone, salt_rotation,
                                  respect_gpc, is_self_site, allowed_origins, allowed_query_keys,
                                  raw_retention_days, created_at_ms, updated_at_ms
                           FROM sites";

pub(crate) fn ensure_default_org_row(conn: &Connection, now_ms: i64) -> Result<Org, CoreError> {
    let existing = conn
        .query_row(
            "SELECT org_id, slug, display_name, created_at_ms, updated_at_ms
             FROM orgs WHERE slug = 'default'",
            [],
            decode_org,
        )
        .optional()
        .map_err(map_sql_err)?;
    if let Some(org) = existing {
        return Ok(org);
    }
    let ts = u64::try_from(now_ms.max(0)).unwrap_or(0);
    let org_id = OrgId::from_uuidv7_parts(ts, [0x01; 10]);
    conn.execute(
        "INSERT OR IGNORE INTO orgs (org_id, slug, display_name, created_at_ms, updated_at_ms)
         VALUES (?1, 'default', 'Default', ?2, ?2)",
        params![org_id.as_bytes().as_slice(), now_ms],
    )
    .map_err(map_sql_err)?;
    conn.query_row(
        "SELECT org_id, slug, display_name, created_at_ms, updated_at_ms
         FROM orgs WHERE slug = 'default'",
        [],
        decode_org,
    )
    .map_err(map_sql_err)
}

pub(crate) fn upsert_site_row(conn: &mut Connection, site: &Site) -> Result<Site, CoreError> {
    let origins_json = serde_json::to_string(&site.allowed_origins)
        .map_err(|err| CoreError::Storage(err.to_string()))?;
    let query_keys_json = serde_json::to_string(&site.allowed_query_keys)
        .map_err(|err| CoreError::Storage(err.to_string()))?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    tx.execute(
        "INSERT INTO sites (
           site_id, org_id, slug, display_name, timezone, salt_rotation,
           respect_gpc, is_self_site, allowed_origins, allowed_query_keys,
           raw_retention_days, created_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(site_id) DO UPDATE SET
           org_id = excluded.org_id,
           slug = excluded.slug,
           display_name = excluded.display_name,
           timezone = excluded.timezone,
           salt_rotation = excluded.salt_rotation,
           respect_gpc = excluded.respect_gpc,
           is_self_site = excluded.is_self_site,
           allowed_origins = excluded.allowed_origins,
           allowed_query_keys = excluded.allowed_query_keys,
           raw_retention_days = excluded.raw_retention_days,
           updated_at_ms = excluded.updated_at_ms",
        params![
            site.site_id.as_bytes().as_slice(),
            site.org_id.as_bytes().as_slice(),
            site.slug,
            site.display_name,
            site.timezone,
            site.salt_rotation.as_str(),
            i64::from(site.respect_gpc),
            i64::from(site.is_self_site),
            origins_json,
            query_keys_json,
            site.raw_retention_days.map(i64::from),
            site.created_at_ms,
            site.updated_at_ms,
        ],
    )
    .map_err(map_sql_err)?;
    let sql = format!("{SITE_SELECT} WHERE site_id = ?1");
    let saved = tx
        .query_row(&sql, [site.site_id.as_bytes().as_slice()], decode_site)
        .map_err(map_sql_err)?;
    tx.commit().map_err(map_sql_err)?;
    Ok(saved)
}

pub(crate) fn resolve_site_row(
    conn: &Connection,
    org_id: OrgId,
    site_ref: &SiteRef,
) -> Result<Option<Site>, CoreError> {
    match site_ref {
        SiteRef::Id(site_id) => {
            let sql = format!("{SITE_SELECT} WHERE org_id = ?1 AND site_id = ?2");
            conn.query_row(
                &sql,
                params![org_id.as_bytes().as_slice(), site_id.as_bytes().as_slice()],
                decode_site,
            )
            .optional()
            .map_err(map_sql_err)
        }
        SiteRef::Slug(slug) => {
            let sql = format!("{SITE_SELECT} WHERE org_id = ?1 AND slug = ?2");
            conn.query_row(
                &sql,
                params![org_id.as_bytes().as_slice(), slug],
                decode_site,
            )
            .optional()
            .map_err(map_sql_err)
        }
    }
}

pub(crate) fn list_site_rows(conn: &Connection, org_id: OrgId) -> Result<Vec<Site>, CoreError> {
    let sql = format!("{SITE_SELECT} WHERE org_id = ?1 ORDER BY created_at_ms ASC");
    let mut stmt = conn.prepare(&sql).map_err(map_sql_err)?;
    let rows = stmt
        .query_map([org_id.as_bytes().as_slice()], decode_site)
        .map_err(map_sql_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_sql_err)
}

pub(crate) fn delete_site_cascade(
    conn: &mut Connection,
    interner: &mut DimInterner,
    site_id: SiteId,
) -> Result<bool, CoreError> {
    let blob = site_id.as_bytes().as_slice();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;
    for table in [
        "events",
        "sessions",
        "rollups_hourly",
        "rollups_daily",
        "dim_strings",
        "api_keys",
    ] {
        let sql = format!("DELETE FROM {table} WHERE site_id = ?1");
        tx.execute(&sql, [blob]).map_err(map_sql_err)?;
    }
    let deleted = tx
        .execute("DELETE FROM sites WHERE site_id = ?1", [blob])
        .map_err(map_sql_err)?;
    tx.commit().map_err(map_sql_err)?;
    if deleted > 0 {
        interner.clear();
    }
    Ok(deleted > 0)
}
