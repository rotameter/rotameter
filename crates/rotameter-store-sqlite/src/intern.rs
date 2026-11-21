#![forbid(unsafe_code)]
//! Site-scoped `dim_strings` integer interner backed by a bounded LRU cache.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::session::lru::BoundedLru;
use rotameter_core::types::DimensionKind;
use rusqlite::{Transaction, params};

use crate::config::map_sql_err;

/// Writer-owned bounded `(site_id, kind, value) -> dim_strings.id` interner.
pub(crate) struct DimInterner {
    cache: BoundedLru<(SiteId, DimensionKind, String), i64>,
}

impl DimInterner {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            cache: BoundedLru::new(capacity),
        }
    }

    pub(crate) fn intern(
        &mut self,
        tx: &Transaction<'_>,
        site_id: SiteId,
        kind: DimensionKind,
        value: &str,
    ) -> Result<i64, CoreError> {
        let key = (site_id, kind, value.to_string());
        if let Some(id) = self.cache.get_mut(&key) {
            return Ok(*id);
        }

        let kind_int = i64::from(kind as u8);
        let site_blob = site_id.as_bytes().as_slice();
        {
            let mut insert = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO dim_strings (site_id, kind, value) VALUES (?1, ?2, ?3)",
                )
                .map_err(map_sql_err)?;
            insert
                .execute(params![site_blob, kind_int, value])
                .map_err(map_sql_err)?;
        }
        let id: i64 = {
            let mut select = tx
                .prepare_cached(
                    "SELECT id FROM dim_strings WHERE site_id = ?1 AND kind = ?2 AND value = ?3",
                )
                .map_err(map_sql_err)?;
            select
                .query_row(params![site_blob, kind_int, value], |row| row.get(0))
                .map_err(map_sql_err)?
        };
        self.cache.insert(key, id);
        Ok(id)
    }

    pub(crate) fn clear(&mut self) {
        self.cache = BoundedLru::new(self.cache.capacity());
    }
}
