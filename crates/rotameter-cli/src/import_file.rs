#![forbid(unsafe_code)]
//! One-shot streaming file import, `--dry-run`, and formatted summary output.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use rotameter_client::types::{BatchEventItem, BatchEventsRequest, MAX_BATCH_EVENTS, SiteRef};

use crate::cli::GlobalOpts;
use crate::cli_sub::ImportArgs;
use crate::context::{prefer_local_db, print_formatted, remote_client};
use crate::exit::CliError;
use crate::import_parse::{
    default_host_for_site, parse_import_file, parse_import_line, parse_import_reader,
};

pub(crate) fn run_dry_run_file(
    global: &GlobalOpts,
    args: &ImportArgs,
    site_ref: &SiteRef,
    path: Option<&Path>,
    apply_proxy_filter: bool,
) -> Result<(), CliError> {
    let parsed = if let Some(p) = path {
        parse_import_file(site_ref, p, args.input_format, apply_proxy_filter)?
    } else {
        let stdin = std::io::stdin();
        parse_import_reader(
            site_ref,
            BufReader::new(stdin.lock()),
            args.input_format,
            apply_proxy_filter,
        )?
    };
    let payload = serde_json::json!({
        "dry_run": true,
        "site": site_ref.to_string(),
        "parsed": parsed.items.len(),
        "skipped": parsed.skipped,
    });
    print_formatted(
        global.format,
        &["DRY_RUN", "SITE", "PARSED", "SKIPPED"],
        &[vec![
            "true".to_string(),
            site_ref.to_string(),
            parsed.items.len().to_string(),
            parsed.skipped.to_string(),
        ]],
        &payload,
        global.quiet,
    )
}

pub(crate) fn run_oneshot_file(
    global: &GlobalOpts,
    args: &ImportArgs,
    site_ref: &SiteRef,
    path: &Path,
    apply_proxy_filter: bool,
) -> Result<(), CliError> {
    let file = File::open(path)
        .map_err(|err| CliError::runtime(format!("failed to open `{}`: {err}", path.display())))?;
    let reader = BufReader::new(file);
    let host = default_host_for_site(site_ref);
    let chunk_cap = args.batch_size.clamp(1, MAX_BATCH_EVENTS);
    let client = if prefer_local_db(global) {
        None
    } else {
        Some(remote_client(global)?)
    };

    let mut buf = Vec::with_capacity(chunk_cap);
    let (mut parsed, mut skipped, mut accepted, mut rejected) = (0_u64, 0_u64, 0_u64, 0_u64);
    for line_res in reader.lines() {
        let line = line_res.map_err(|err| {
            CliError::runtime(format!("failed reading `{}`: {err}", path.display()))
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match parse_import_line(
            site_ref,
            &host,
            trimmed,
            args.input_format,
            apply_proxy_filter,
        ) {
            Some(item) => {
                parsed += 1;
                buf.push(item);
                if buf.len() >= chunk_cap {
                    let (a, r) = flush_chunk(global, site_ref, client.as_ref(), &mut buf)?;
                    accepted += a;
                    rejected += r;
                }
            }
            None => skipped += 1,
        }
    }
    if !buf.is_empty() {
        let (a, r) = flush_chunk(global, site_ref, client.as_ref(), &mut buf)?;
        accepted += a;
        rejected += r;
    }

    print_import_summary(global, site_ref, parsed, skipped, accepted, rejected, 0, 0)
}

fn flush_chunk(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    client: Option<&rotameter_client::BlockingRotameterClient>,
    buf: &mut Vec<BatchEventItem>,
) -> Result<(u64, u64), CliError> {
    let chunk = std::mem::take(buf);
    if let Some(c) = client {
        let res = c.post_events_batch(&BatchEventsRequest { events: chunk })?;
        Ok((res.accepted, res.rejected))
    } else {
        #[cfg(feature = "sqlite")]
        {
            crate::local_import::ingest_local(global, site_ref, &chunk)
        }
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (global, site_ref, chunk);
            Err(CliError::usage(
                "local `--db` import requires the `sqlite` feature",
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_import_summary(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    parsed: u64,
    skipped: u64,
    accepted: u64,
    rejected: u64,
    dropped_backpressure: u64,
    dropped_upstream: u64,
) -> Result<(), CliError> {
    let payload = serde_json::json!({
        "site": site_ref.to_string(),
        "parsed": parsed,
        "skipped": skipped,
        "accepted": accepted,
        "rejected": rejected,
        "dropped_backpressure": dropped_backpressure,
        "dropped_upstream": dropped_upstream,
    });
    print_formatted(
        global.format,
        &["SITE", "PARSED", "SKIPPED", "ACCEPTED", "REJECTED"],
        &[vec![
            site_ref.to_string(),
            parsed.to_string(),
            skipped.to_string(),
            accepted.to_string(),
            rejected.to_string(),
        ]],
        &payload,
        global.quiet,
    )
}
