#![forbid(unsafe_code)]
//! Slug validation and `SiteRef` parsing (`site_id` or `slug`) per `ADR-0007`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::id::SiteId;

/// Maximum byte length allowed for an org or site `slug`.
pub const MAX_SLUG_BYTES: usize = 128;

/// Validates and normalizes an organization or site slug into lowercase ASCII.
///
/// Allows lowercase `[a-z0-9._-]`, permits the built-in `_self` dogfood slug, and rejects
/// empty strings, whitespace, or leading/trailing `.` or `-`.
///
/// # Errors
///
/// Returns [`CoreError::InvalidSlug`] if `raw` violates length or character rules.
pub fn validate_slug(raw: &str) -> Result<String, CoreError> {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > MAX_SLUG_BYTES {
        return Err(CoreError::InvalidSlug(format!(
            "slug length must be 1..={MAX_SLUG_BYTES} bytes"
        )));
    }
    if normalized != "_self"
        && (normalized.starts_with(['.', '-']) || normalized.ends_with(['.', '-']))
    {
        return Err(CoreError::InvalidSlug(normalized));
    }
    if !normalized
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(CoreError::InvalidSlug(normalized));
    }
    Ok(normalized)
}

/// Reference to a site accepted across API, CLI, MCP, and TUI (`site_...` UID or unique `slug`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SiteRef {
    /// Immutable prefixed `UUIDv7` site identifier.
    Id(SiteId),
    /// Human-friendly org-unique slug (e.g., `colinwolff.dev` or `_self`).
    Slug(String),
}

impl SiteRef {
    /// Parses `raw` as a [`SiteRef::Id`] if it starts with `site_`, otherwise as a [`SiteRef::Slug`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidId`] or [`CoreError::InvalidSlug`] on malformed input.
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let trimmed = raw.trim();
        if trimmed.starts_with("site_") {
            Ok(Self::Id(SiteId::from_str(trimmed)?))
        } else {
            Ok(Self::Slug(validate_slug(trimmed)?))
        }
    }
}

impl FromStr for SiteRef {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for SiteRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => id.fmt(f),
            Self::Slug(slug) => f.write_str(slug),
        }
    }
}

impl Serialize for SiteRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SiteRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}
