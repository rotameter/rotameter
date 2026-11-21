#![forbid(unsafe_code)]
//! Event kind, bounded custom properties, and UTM domain types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::normalize::truncate_utf8;

/// Maximum normalized URL path+query byte length stored per event.
pub const MAX_PATH_BYTES: usize = 512;
/// Maximum byte length for custom event names.
pub const MAX_EVENT_NAME_BYTES: usize = 64;
/// Maximum number of scalar custom property entries per event.
pub const MAX_PROPS_KEYS: usize = 10;
/// Maximum byte length per custom property key.
pub const MAX_PROP_KEY_BYTES: usize = 64;
/// Maximum byte length per custom property scalar string value.
pub const MAX_PROP_VALUE_BYTES: usize = 128;
/// Maximum byte length per extracted `utm_*` field.
pub const MAX_UTM_BYTES: usize = 128;
/// Maximum byte length inspected from incoming `User-Agent` headers.
pub const MAX_USER_AGENT_BYTES: usize = 512;

/// Built-in or validated custom analytics event kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum EventKind {
    /// Standard page impression (`pageview`).
    Pageview,
    /// Client- or SSR-reported 404 route hit (`not_found`).
    NotFound,
    /// Outbound external `<a href>` click (`outbound_click`).
    OutboundClick,
    /// Downloadable asset click (`file_download`).
    FileDownload,
    /// Validated custom application event.
    Custom(String),
}

impl EventKind {
    /// Parses and validates an event name against `1..=64` ASCII `[a-zA-Z0-9_.:-]`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidEventName`] if empty, oversized, or containing disallowed bytes.
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_EVENT_NAME_BYTES {
            return Err(CoreError::InvalidEventName(format!(
                "event name length must be 1..={MAX_EVENT_NAME_BYTES} bytes"
            )));
        }
        if !trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
        {
            return Err(CoreError::InvalidEventName(trimmed.to_string()));
        }
        Ok(match trimmed {
            "pageview" => Self::Pageview,
            "not_found" => Self::NotFound,
            "outbound_click" => Self::OutboundClick,
            "file_download" => Self::FileDownload,
            other => Self::Custom(other.to_string()),
        })
    }

    /// Returns the wire string representation of the event name.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pageview => "pageview",
            Self::NotFound => "not_found",
            Self::OutboundClick => "outbound_click",
            Self::FileDownload => "file_download",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl TryFrom<String> for EventKind {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<EventKind> for String {
    fn from(value: EventKind) -> Self {
        value.as_str().to_string()
    }
}

/// Bounded map of scalar custom event properties (`<= 10` keys, capped key/value bytes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, String>")]
pub struct EventProps(BTreeMap<String, String>);

impl EventProps {
    /// Constructs bounded event properties, truncating scalar values safely at `128` UTF-8 bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidProperties`] if key count exceeds [`MAX_PROPS_KEYS`] or any key
    /// is empty or exceeds [`MAX_PROP_KEY_BYTES`].
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, CoreError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut map = BTreeMap::new();
        for (raw_key, raw_val) in pairs {
            let key = raw_key.as_ref().trim();
            if key.is_empty() || key.len() > MAX_PROP_KEY_BYTES {
                return Err(CoreError::InvalidProperties(format!(
                    "property key length must be 1..={MAX_PROP_KEY_BYTES} bytes"
                )));
            }
            if !map.contains_key(key) && map.len() >= MAX_PROPS_KEYS {
                return Err(CoreError::InvalidProperties(format!(
                    "at most {MAX_PROPS_KEYS} property keys allowed"
                )));
            }
            let value = truncate_utf8(raw_val.as_ref().trim(), MAX_PROP_VALUE_BYTES).to_string();
            map.insert(key.to_string(), value);
        }
        Ok(Self(map))
    }

    /// Returns the underlying ordered property map.
    pub const fn as_map(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    /// Returns the number of stored custom properties.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` when no custom properties are attached.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<BTreeMap<String, String>> for EventProps {
    type Error = CoreError;

    fn try_from(value: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        Self::from_pairs(value)
    }
}

/// Extracted `utm_*` campaign parameters (`MAX_UTM_BYTES` cap each).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtmParams {
    /// `utm_source` value, if present.
    pub source: Option<String>,
    /// `utm_medium` value, if present.
    pub medium: Option<String>,
    /// `utm_campaign` value, if present.
    pub campaign: Option<String>,
    /// `utm_content` value, if present.
    pub content: Option<String>,
    /// `utm_term` value, if present.
    pub term: Option<String>,
}
