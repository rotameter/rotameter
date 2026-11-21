#![forbid(unsafe_code)]
//! Strongly typed error codes and domain validation errors for `rotameter-core`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable machine-readable error codes matching the `/v1/*` HTTP error envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Malformed or unrecognized prefixed `UUIDv7` identifier.
    InvalidId,
    /// Invalid organization or site slug.
    InvalidSlug,
    /// Malformed or unsupported URL.
    InvalidUrl,
    /// Invalid event name outside `[a-zA-Z0-9_.:-]` or byte limit.
    InvalidEventName,
    /// Event custom properties exceeded key/size constraints.
    InvalidProperties,
    /// Query specification failed semantic validation.
    InvalidQuery,
    /// Request URL host is not permitted by site `allowed_origins`.
    HostMismatch,
    /// Persistence backend failure surfaced through a storage trait.
    StorageError,
    /// Salt lookup or lock acquisition failed.
    SaltUnavailable,
}

/// Pure-Rust domain validation error returned by `rotameter-core` constructors and parsers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// Identifier string failed prefix or Crockford base32 `UUIDv7` validation.
    #[error("invalid identifier: {0}")]
    InvalidId(String),
    /// Slug failed normalization or character/length rules.
    #[error("invalid slug: {0}")]
    InvalidSlug(String),
    /// URL failed HTTP(S) parsing or host validation.
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    /// Event name failed ASCII character set or length validation.
    #[error("invalid event name: {0}")]
    InvalidEventName(String),
    /// Event custom properties failed key count or length validation.
    #[error("invalid event properties: {0}")]
    InvalidProperties(String),
    /// Query specification failed semantic validation.
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    /// URL host is outside the site's `allowed_origins`.
    #[error("host `{0}` is not permitted for this site")]
    HostMismatch(String),
    /// Storage trait operation failed.
    #[error("storage error: {0}")]
    Storage(String),
    /// Salt provider could not resolve a window salt.
    #[error("salt unavailable: {0}")]
    SaltUnavailable(String),
}

impl CoreError {
    /// Returns the stable machine-readable [`ErrorCode`] corresponding to this error.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidId(_) => ErrorCode::InvalidId,
            Self::InvalidSlug(_) => ErrorCode::InvalidSlug,
            Self::InvalidUrl(_) => ErrorCode::InvalidUrl,
            Self::InvalidEventName(_) => ErrorCode::InvalidEventName,
            Self::InvalidProperties(_) => ErrorCode::InvalidProperties,
            Self::InvalidQuery(_) => ErrorCode::InvalidQuery,
            Self::HostMismatch(_) => ErrorCode::HostMismatch,
            Self::Storage(_) => ErrorCode::StorageError,
            Self::SaltUnavailable(_) => ErrorCode::SaltUnavailable,
        }
    }
}
