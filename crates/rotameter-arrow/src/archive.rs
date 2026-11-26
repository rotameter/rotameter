#![forbid(unsafe_code)]
//! Deterministic `.tar.zst` pack and path-traversal-safe unpack helpers for portable backups.

use std::fs::{self, File};
use std::path::{Component, Path};

use crate::ArrowError;

/// Packs all files under `src_dir` into a single `Zstd`-compressed tar archive at `archive_path`.
///
/// # Errors
///
/// Returns [`ArrowError`] if parent directory creation, tar packing, or zstd finalization fails.
pub fn pack_tar_zst(src_dir: &Path, archive_path: &Path) -> Result<(), ArrowError> {
    if let Some(parent) = archive_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let out = File::create(archive_path)?;
    let encoder = zstd::stream::write::Encoder::new(out, 3)?;
    let mut tar_builder = tar::Builder::new(encoder);
    tar_builder.append_dir_all(".", src_dir)?;
    let encoder = tar_builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}

/// Unpacks `archive_path` (`.tar.zst`) into `dst_dir`, rejecting path traversal or symlink escapes.
///
/// # Errors
///
/// Returns [`ArrowError`] on decompression errors, tar read errors, or unsafe archive entries.
pub fn unpack_tar_zst(archive_path: &Path, dst_dir: &Path) -> Result<(), ArrowError> {
    fs::create_dir_all(dst_dir)?;
    let file = File::open(archive_path)?;
    let decoder = zstd::stream::read::Decoder::new(file)?;
    let mut archive = tar::Archive::new(decoder);

    for entry_res in archive.entries()? {
        let mut entry = entry_res?;
        let rel_path = entry.path()?.into_owned();
        if !is_safe_rel_path(&rel_path) {
            return Err(ArrowError::InvalidExchange(format!(
                "unsafe archive path rejected: `{}`",
                rel_path.display()
            )));
        }
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(ArrowError::InvalidExchange(format!(
                "archive link entry rejected: `{}`",
                rel_path.display()
            )));
        }
        let dest = dst_dir.join(&rel_path);
        if kind.is_dir() {
            fs::create_dir_all(&dest)?;
        } else if kind.is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(&dest)?;
        }
    }
    Ok(())
}

fn is_safe_rel_path(path: &Path) -> bool {
    !path.is_absolute()
        && !path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}
