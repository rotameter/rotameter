#![forbid(unsafe_code)]
//! Build script embedding pre-built `ui/dist/` static assets when `--features ui` is active.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_EMBEDDED_BYTES: usize = 1_572_864;
const PLACEHOLDER_HTML: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Rotameter</title></head><body><main id=\"app\">Rotameter Web UI bundle is not built yet.</main></body></html>";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_UI");
    if env::var_os("CARGO_FEATURE_UI").is_none() {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let dist_dir = manifest_dir.join("../../ui/dist");
    println!("cargo:rerun-if-changed={}", dist_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
    let out_file = out_dir.join("embedded_ui.rs");

    let mut files = Vec::new();
    if dist_dir.is_dir() && dist_dir.join("index.html").is_file() {
        collect_files(&dist_dir, &dist_dir, &mut files);
        files.sort_by(|a, b| a.0.cmp(&b.0));
    }

    if files.is_empty() {
        let content = format!(
            "pub(crate) static EMBEDDED_ASSETS: &[(&str, &[u8])] = &[\n    (\"index.html\", br#\"{PLACEHOLDER_HTML}\"#),\n];\n"
        );
        fs::write(&out_file, content)?;
        return Ok(());
    }

    let mut total_bytes: usize = 0;
    let mut code = String::from("pub(crate) static EMBEDDED_ASSETS: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in files {
        let len = fs::metadata(&abs).map_or(0, |m| usize::try_from(m.len()).unwrap_or(usize::MAX));
        total_bytes = total_bytes.saturating_add(len);
        let escaped_rel = rel.escape_default();
        let escaped_abs = abs.to_string_lossy().escape_default().to_string();
        let _ = writeln!(
            code,
            "    (\"{escaped_rel}\", include_bytes!(\"{escaped_abs}\")),"
        );
    }
    code.push_str("];\n");

    if total_bytes > MAX_EMBEDDED_BYTES {
        return Err(format!(
            "embedded ui/dist exceeds 1.5 MiB budget ({total_bytes} > {MAX_EMBEDDED_BYTES} bytes)"
        )
        .into());
    }
    fs::write(&out_file, code)?;
    Ok(())
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if path.is_file()
            && let Ok(rel) = path.strip_prefix(root)
            && let Ok(abs) = path.canonicalize()
        {
            let norm = rel.to_string_lossy().replace('\\', "/");
            out.push((norm, abs));
        }
    }
}
