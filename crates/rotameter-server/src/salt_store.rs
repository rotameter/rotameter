#![forbid(unsafe_code)]
//! Cached database-backed and in-memory rotating salt resolution with grace-window pruning.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{
    GRACE_WINDOW_MS, InMemorySaltProvider, SaltBytes, SaltProvider, SaltWindow, VisitorCandidates,
    compute_visitor_id, resolve_salt_window,
};
use rotameter_core::storage::ControlPlaneStore;
use rotameter_core::types::SaltRotation;

use crate::config::SaltStoreMode;
use crate::state::{fill_random, now_ms};

const ALL_ROTATIONS: [SaltRotation; 3] = [
    SaltRotation::Daily,
    SaltRotation::Weekly,
    SaltRotation::Monthly,
];

#[derive(Clone)]
pub(crate) enum ServerSaltStore {
    Db(Arc<DbSaltStore>),
    Memory(Arc<InMemorySaltProvider>),
}

pub(crate) struct DbSaltStore {
    control: Arc<dyn ControlPlaneStore>,
    state: RwLock<SaltCacheState>,
}

#[derive(Default)]
struct SaltCacheState {
    salts: HashMap<(SaltRotation, i64), SaltBytes>,
    pruned_before: [Option<i64>; 3],
}

impl ServerSaltStore {
    pub(crate) fn new(mode: SaltStoreMode, control: Arc<dyn ControlPlaneStore>) -> Self {
        match mode {
            SaltStoreMode::Db => Self::Db(Arc::new(DbSaltStore {
                control,
                state: RwLock::new(SaltCacheState::default()),
            })),
            SaltStoreMode::Memory => Self::Memory(Arc::new(InMemorySaltProvider::new(|_| {
                random_salt_fallback()
            }))),
        }
    }

    pub(crate) async fn prune_expired_all(&self, current_ms: i64) -> Result<(), CoreError> {
        let Self::Db(db) = self else {
            return Ok(());
        };
        for rotation in ALL_ROTATIONS {
            let min_keep = min_keep_window_id(rotation, current_ms);
            db.prune_if_needed(rotation, min_keep).await?;
        }
        Ok(())
    }

    pub(crate) async fn resolve_candidates(
        &self,
        site_id: SiteId,
        rotation: SaltRotation,
        ts_ms: i64,
        client_ip: &str,
        user_agent: &str,
    ) -> Result<VisitorCandidates, CoreError> {
        let windows = resolve_salt_window(rotation, ts_ms);
        match self {
            Self::Memory(mem) => {
                let cur_salt = mem.salt_for(windows.current)?;
                let current = compute_visitor_id(&cur_salt, site_id, client_ip, user_agent);
                let previous = match windows.previous_in_grace {
                    Some(prev) => {
                        let prev_salt = mem.salt_for(prev)?;
                        Some(compute_visitor_id(
                            &prev_salt, site_id, client_ip, user_agent,
                        ))
                    }
                    None => None,
                };
                Ok(VisitorCandidates { current, previous })
            }
            Self::Db(db) => {
                let wall_now = now_ms();
                let cur_salt = db.salt_for(windows.current, wall_now).await?;
                let current = compute_visitor_id(&cur_salt, site_id, client_ip, user_agent);
                let previous = match windows.previous_in_grace {
                    Some(prev) => {
                        let prev_salt = db.salt_for(prev, wall_now).await?;
                        Some(compute_visitor_id(
                            &prev_salt, site_id, client_ip, user_agent,
                        ))
                    }
                    None => None,
                };
                let min_keep = min_keep_window_id(rotation, wall_now);
                if let Err(err) = db.prune_if_needed(rotation, min_keep).await {
                    tracing::warn!(?rotation, "failed to prune expired rotating salts: {err}");
                }
                Ok(VisitorCandidates { current, previous })
            }
        }
    }
}

impl DbSaltStore {
    async fn salt_for(&self, window: SaltWindow, wall_now_ms: i64) -> Result<SaltBytes, CoreError> {
        if let Ok(guard) = self.state.read()
            && let Some(cached) = guard.salts.get(&(window.rotation, window.window_id))
        {
            return Ok(*cached);
        }

        let candidate = fill_random::<32>()?;
        let persisted = self
            .control
            .get_or_create_salt(window, candidate, wall_now_ms)
            .await?;

        if let Ok(mut guard) = self.state.write() {
            guard
                .salts
                .insert((window.rotation, window.window_id), persisted);
        }
        Ok(persisted)
    }

    async fn prune_if_needed(
        &self,
        rotation: SaltRotation,
        min_keep_window_id: i64,
    ) -> Result<(), CoreError> {
        let idx = rotation_index(rotation);
        if let Ok(guard) = self.state.read()
            && guard
                .pruned_before
                .get(idx)
                .and_then(|val| *val)
                .is_some_and(|prev| prev >= min_keep_window_id)
        {
            return Ok(());
        }

        self.control
            .prune_salts_before(rotation, min_keep_window_id)
            .await?;

        if let Ok(mut guard) = self.state.write() {
            guard
                .salts
                .retain(|(rot, win_id), _| *rot != rotation || *win_id >= min_keep_window_id);
            if let Some(slot) = guard.pruned_before.get_mut(idx) {
                *slot = Some(min_keep_window_id);
            }
        }
        Ok(())
    }
}

fn min_keep_window_id(rotation: SaltRotation, current_ms: i64) -> i64 {
    resolve_salt_window(rotation, current_ms.saturating_sub(GRACE_WINDOW_MS))
        .current
        .window_id
}

const fn rotation_index(rotation: SaltRotation) -> usize {
    match rotation {
        SaltRotation::Daily => 0,
        SaltRotation::Weekly => 1,
        SaltRotation::Monthly => 2,
    }
}

fn random_salt_fallback() -> SaltBytes {
    fill_random::<32>().unwrap_or_else(|_| {
        let now = now_ms().to_be_bytes();
        let mut out = [0_u8; 32];
        out[..8].copy_from_slice(&now);
        out
    })
}
