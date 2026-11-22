#![forbid(unsafe_code)]
//! `rotameter import` command handler (`JSONL` and `CLF` with `--dry-run`).

use rotameter_client::types::{BatchEventsRequest, MAX_BATCH_EVENTS, SiteRef};

use crate::cli::GlobalOpts;
use crate::cli_sub::ImportArgs;
use crate::context::{prefer_local_db, print_formatted, remote_client};
use crate::exit::CliError;
use crate::import_parse::parse_import_file;

pub(crate) fn run_import(global: &GlobalOpts, args: &ImportArgs) -> Result<(), CliError> {
    let site_ref = SiteRef::parse(&args.site)?;
    let parsed = parse_import_file(&site_ref, &args.file, args.input_format)?;

    if args.dry_run {
        let payload = serde_json::json!({
            "dry_run": true,
            "site": site_ref.to_string(),
            "parsed": parsed.items.len(),
            "skipped": parsed.skipped,
        });
        return print_formatted(
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
        );
    }

    let (accepted, rejected) = if prefer_local_db(global) {
        #[cfg(feature = "sqlite")]
        {
            crate::local_import::ingest_local(global, &site_ref, &parsed.items)?
        }
        #[cfg(not(feature = "sqlite"))]
        {
            return Err(CliError::usage(
                "local `--db` import requires the `sqlite` feature",
            ));
        }
    } else {
        let client = remote_client(global)?;
        let chunk_size = args.batch_size.clamp(1, MAX_BATCH_EVENTS);
        let (mut acc, mut rej) = (0_u64, 0_u64);
        for chunk in parsed.items.chunks(chunk_size) {
            let res = client.post_events_batch(&BatchEventsRequest {
                events: chunk.to_vec(),
            })?;
            acc += res.accepted;
            rej += res.rejected;
        }
        (acc, rej)
    };

    let payload = serde_json::json!({
        "site": site_ref.to_string(),
        "parsed": parsed.items.len(),
        "skipped": parsed.skipped,
        "accepted": accepted,
        "rejected": rejected,
    });
    print_formatted(
        global.format,
        &["SITE", "PARSED", "SKIPPED", "ACCEPTED", "REJECTED"],
        &[vec![
            site_ref.to_string(),
            parsed.items.len().to_string(),
            parsed.skipped.to_string(),
            accepted.to_string(),
            rejected.to_string(),
        ]],
        &payload,
        global.quiet,
    )
}
