#![forbid(unsafe_code)]
//! TypeID-style prefixed `UUIDv7` identifiers (`org_...`, `site_...`, `key_...`) per `ADR-0007`.

mod base32;
mod slug;

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

pub use slug::{MAX_SLUG_BYTES, SiteRef, validate_slug};

/// Assembles a standard RFC 9562 `UUIDv7` 16-byte array from a 48-bit Unix ms timestamp and entropy.
pub fn build_uuidv7_bytes(timestamp_ms: u64, rand_bytes: [u8; 10]) -> [u8; 16] {
    let ts = timestamp_ms.to_be_bytes();
    let mut bytes = [0u8; 16];
    bytes[0..6].copy_from_slice(&ts[2..8]);
    bytes[6] = 0x70 | (rand_bytes[0] & 0x0f);
    bytes[7] = rand_bytes[1];
    bytes[8] = 0x80 | (rand_bytes[2] & 0x3f);
    bytes[9..16].copy_from_slice(&rand_bytes[3..10]);
    bytes
}

/// Extracts the 48-bit millisecond timestamp prefix from `UUIDv7` bytes.
pub fn extract_timestamp_ms(bytes: &[u8; 16]) -> u64 {
    let mut ts = [0u8; 8];
    ts[2..8].copy_from_slice(&bytes[0..6]);
    u64::from_be_bytes(ts)
}

macro_rules! define_prefixed_id {
    ($name:ident, $prefix:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Wire string prefix for this identifier kind.
            pub const PREFIX: &'static str = $prefix;

            /// Constructs an identifier directly from raw 16-byte binary storage.
            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            /// Constructs an identifier from a 48-bit Unix ms timestamp and 80 bits of entropy.
            pub fn from_uuidv7_parts(timestamp_ms: u64, rand_bytes: [u8; 10]) -> Self {
                Self(build_uuidv7_bytes(timestamp_ms, rand_bytes))
            }

            /// Returns the underlying 16-byte big-endian `UUIDv7` representation.
            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            /// Returns the embedded 48-bit millisecond timestamp.
            pub fn timestamp_ms(&self) -> u64 {
                extract_timestamp_ms(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}_{}", Self::PREFIX, base32::encode(self.0))
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let expected_prefix = concat!($prefix, "_");
                let suffix = s.strip_prefix(expected_prefix).ok_or_else(|| {
                    CoreError::InvalidId(format!("expected prefix `{expected_prefix}` in `{s}`"))
                })?;
                let bytes = base32::decode(suffix).ok_or_else(|| {
                    CoreError::InvalidId(format!("invalid base32 UUIDv7 payload in `{s}`"))
                })?;
                Ok(Self(bytes))
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                Self::from_str(&raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_prefixed_id!(
    OrgId,
    "org",
    "Immutable TypeID-style organization identifier (`org_<26-char base32>`)."
);
define_prefixed_id!(
    SiteId,
    "site",
    "Immutable TypeID-style site identifier (`site_<26-char base32>`)."
);
define_prefixed_id!(
    KeyId,
    "key",
    "Immutable TypeID-style API key identifier (`key_<26-char base32>`)."
);
