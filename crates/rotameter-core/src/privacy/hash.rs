#![forbid(unsafe_code)]
//! `HMAC-SHA256` visitor hashing, `SHA-256` API key hashing, and pluggable [`SaltProvider`] seam.

use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use hmac_sha256::{HMAC, Hash};

use crate::error::CoreError;
use crate::id::SiteId;
use crate::normalize::truncate_utf8;
use crate::privacy::window::{SaltWindow, resolve_salt_window};
use crate::types::{MAX_USER_AGENT_BYTES, SaltRotation};

/// `32`-byte rotating secret salt used for pseudonymous visitor hashing.
pub type SaltBytes = [u8; 32];
/// `16`-byte truncated `HMAC-SHA256` visitor identifier.
pub type VisitorId = [u8; 16];
/// `16`-byte session identifier.
pub type SessionId = [u8; 16];

/// Pluggable rotating salt provider (`ADR-0007` multi-node / `CSPRNG` seam).
pub trait SaltProvider: Send + Sync {
    /// Returns the `32`-byte salt assigned to `window`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SaltUnavailable`] if the salt cannot be loaded or generated.
    fn salt_for(&self, window: SaltWindow) -> Result<SaltBytes, CoreError>;
}

/// Process-local in-memory [`SaltProvider`] backed by a caller-supplied salt generator.
pub struct InMemorySaltProvider {
    salts: RwLock<HashMap<(SaltRotation, i64), SaltBytes>>,
    generator: Box<dyn Fn(SaltWindow) -> SaltBytes + Send + Sync>,
}

impl InMemorySaltProvider {
    /// Creates an in-memory provider that lazily mints missing window salts using `generator`.
    pub fn new(generator: impl Fn(SaltWindow) -> SaltBytes + Send + Sync + 'static) -> Self {
        Self {
            salts: RwLock::new(HashMap::new()),
            generator: Box::new(generator),
        }
    }

    /// Pre-populates or overrides the salt stored for `window`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SaltUnavailable`] if the internal lock is poisoned.
    pub fn insert_salt(&self, window: SaltWindow, salt: SaltBytes) -> Result<(), CoreError> {
        let mut guard = self
            .salts
            .write()
            .map_err(|err| CoreError::SaltUnavailable(err.to_string()))?;
        guard.insert((window.rotation, window.window_id), salt);
        Ok(())
    }

    /// Drops cached salts for `rotation` strictly older than `min_window_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SaltUnavailable`] if the internal lock is poisoned.
    pub fn prune_before(
        &self,
        rotation: SaltRotation,
        min_window_id: i64,
    ) -> Result<(), CoreError> {
        let mut guard = self
            .salts
            .write()
            .map_err(|err| CoreError::SaltUnavailable(err.to_string()))?;
        guard.retain(|&(rot, id), _| rot != rotation || id >= min_window_id);
        Ok(())
    }
}

impl SaltProvider for InMemorySaltProvider {
    fn salt_for(&self, window: SaltWindow) -> Result<SaltBytes, CoreError> {
        let key = (window.rotation, window.window_id);
        {
            let guard = self
                .salts
                .read()
                .map_err(|err| CoreError::SaltUnavailable(err.to_string()))?;
            if let Some(salt) = guard.get(&key) {
                return Ok(*salt);
            }
        }
        let mut guard = self
            .salts
            .write()
            .map_err(|err| CoreError::SaltUnavailable(err.to_string()))?;
        Ok(*guard.entry(key).or_insert_with(|| (self.generator)(window)))
    }
}

impl fmt::Debug for InMemorySaltProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemorySaltProvider")
            .finish_non_exhaustive()
    }
}

/// Computes the `16`-byte pseudonymous `visitor_id` per `SPEC-PRIVACY.md` §2.
pub fn compute_visitor_id(
    salt: &SaltBytes,
    site_id: SiteId,
    client_ip: &str,
    user_agent: &str,
) -> VisitorId {
    let mut mac = HMAC::new(salt);
    let capped_ua = truncate_utf8(user_agent, MAX_USER_AGENT_BYTES);
    mac.update(site_id.as_bytes());
    mac.update([0x00]);
    mac.update(client_ip.as_bytes());
    mac.update([0x00]);
    mac.update(capped_ua.as_bytes());

    let digest = mac.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

/// Computes the `32`-byte `SHA-256` digest stored for Bearer API keys.
pub fn compute_api_key_hash(token: &str) -> [u8; 32] {
    Hash::hash(token.as_bytes())
}

/// Current-window and optional grace-window candidate visitor hashes for session lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisitorCandidates {
    /// Hash derived from the active salt window at event time.
    pub current: VisitorId,
    /// Hash derived from the previous window when within the `30m` rollover grace period.
    pub previous: Option<VisitorId>,
}

/// Computes [`VisitorCandidates`] for a request, including previous-window lookup during rollover.
///
/// # Errors
///
/// Returns [`CoreError::SaltUnavailable`] if `provider` fails to resolve either salt window.
pub fn resolve_visitor_candidates(
    provider: &dyn SaltProvider,
    site_id: SiteId,
    rotation: SaltRotation,
    ts_ms: i64,
    client_ip: &str,
    user_agent: &str,
) -> Result<VisitorCandidates, CoreError> {
    let resolved = resolve_salt_window(rotation, ts_ms);
    let current_salt = provider.salt_for(resolved.current)?;
    let current = compute_visitor_id(&current_salt, site_id, client_ip, user_agent);
    let previous = match resolved.previous_in_grace {
        Some(prev_win) => {
            let prev_salt = provider.salt_for(prev_win)?;
            Some(compute_visitor_id(
                &prev_salt, site_id, client_ip, user_agent,
            ))
        }
        None => None,
    };
    Ok(VisitorCandidates { current, previous })
}
