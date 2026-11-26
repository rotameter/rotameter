#![forbid(unsafe_code)]
//! Portable exchange `manifest.json` model and streaming `SHA-256` integrity verification.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use hmac_sha256::Hash;
use serde::{Deserialize, Serialize};

use crate::ArrowError;

/// Version of the on-disk portable backup directory / archive format (`1`).
pub const PORTABLE_FORMAT_VERSION: u32 = 1;

/// Row counters recorded in [`PortableManifest`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableRowCounts {
    /// Organizations exported in `control_plane.json`.
    pub orgs: u64,
    /// Tracked sites exported in `control_plane.json`.
    pub sites: u64,
    /// API keys exported in `control_plane.json`.
    pub api_keys: u64,
    /// Active rotating salts exported in `control_plane.json`.
    pub rotating_salts: u64,
    /// Raw events across `events/day=*.parquet`.
    pub events: u64,
    /// Sliding sessions across `sessions/day=*.parquet`.
    pub sessions: u64,
    /// Hourly rollup rows in `rollups_hourly.parquet`.
    pub rollups_hourly: u64,
    /// Daily rollup rows in `rollups_daily.parquet`.
    pub rollups_daily: u64,
    /// Path daily rollup rows in `rollups_path_daily.parquet`.
    pub rollups_path_daily: u64,
}

/// Self-describing exchange manifest persisted as `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortableManifest {
    /// Portable exchange container format version (`1`).
    pub format_version: u32,
    /// Producer `rotameter` version string.
    pub rotameter_version: String,
    /// Source database schema version at export time.
    pub schema_version: u32,
    /// Canonical Arrow schema version (`1`).
    pub arrow_schema_version: u32,
    /// Export timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Row counters for control-plane and analytics datasets.
    pub row_counts: PortableRowCounts,
    /// Lowercase hex `SHA-256` digest per relative file path inside the export root.
    pub files: BTreeMap<String, String>,
}

/// Computes lowercase hex `SHA-256` digest of `path` using streaming reads.
///
/// # Errors
///
/// Returns [`ArrowError::Io`] if opening or reading `path` fails.
pub fn sha256_file(path: &Path) -> Result<String, ArrowError> {
    let mut file = File::open(path)?;
    let mut hasher = Hash::new();
    let mut buf = [0_u8; 16 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

/// Writes `manifest.json` cleanly inside `dir`.
///
/// # Errors
///
/// Returns [`ArrowError`] if JSON formatting or disk write fails.
pub fn write_manifest(dir: &Path, manifest: &PortableManifest) -> Result<(), ArrowError> {
    let body = serde_json::to_vec_pretty(manifest)?;
    fs::write(dir.join("manifest.json"), body)?;
    Ok(())
}

/// Reads and verifies every `SHA-256` checksum listed in `dir/manifest.json`.
///
/// # Errors
///
/// Returns [`ArrowError::InvalidExchange`] on version mismatch, unsafe relative path,
/// missing file, or `SHA-256` checksum discrepancy.
pub fn verify_manifest(dir: &Path) -> Result<PortableManifest, ArrowError> {
    let raw = fs::read(dir.join("manifest.json"))?;
    let manifest: PortableManifest = serde_json::from_slice(&raw)?;
    if manifest.format_version != PORTABLE_FORMAT_VERSION {
        return Err(ArrowError::InvalidExchange(format!(
            "unsupported portable format version {} (expected {PORTABLE_FORMAT_VERSION})",
            manifest.format_version
        )));
    }
    for (rel, expected) in &manifest.files {
        let rel_path = Path::new(rel);
        if rel_path.is_absolute()
            || rel_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(ArrowError::InvalidExchange(format!(
                "unsafe relative path in manifest: `{rel}`"
            )));
        }
        let target = dir.join(rel_path);
        let actual = sha256_file(&target)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(ArrowError::InvalidExchange(format!(
                "SHA-256 mismatch for `{rel}`: expected {expected}, got {actual}"
            )));
        }
    }
    Ok(manifest)
}
