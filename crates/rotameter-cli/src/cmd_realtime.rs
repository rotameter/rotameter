#![forbid(unsafe_code)]
//! `rotameter realtime` subcommand handler with `--watch` polling support.

use std::time::Duration;

use rotameter_client::types::SiteRef;

use crate::cli::GlobalOpts;
use crate::cli_sub::RealtimeArgs;
use crate::context::{print_formatted, remote_client};
use crate::exit::CliError;
use crate::format::OutputFormat;

pub(crate) fn run_realtime(global: &GlobalOpts, args: &RealtimeArgs) -> Result<(), CliError> {
    let client = remote_client(global)?;
    let site_ref = SiteRef::parse(&args.site)?;
    let max_ticks = if args.watch { args.max_ticks } else { Some(1) };
    let mut tick = 0_u32;

    loop {
        let snap = client.realtime(&site_ref)?;
        let rows: Vec<Vec<String>> = snap
            .active_pages
            .iter()
            .map(|p| vec![p.path.clone(), p.active_visitors.to_string()])
            .collect();
        if global.format == OutputFormat::Table && !global.quiet {
            println!(
                "{} ({}) — {} active visitors",
                snap.slug, snap.site_id, snap.active_visitors
            );
        }
        print_formatted(
            global.format,
            &["PATH", "ACTIVE_VISITORS"],
            &rows,
            &snap,
            global.quiet,
        )?;
        tick += 1;
        if let Some(limit) = max_ticks
            && tick >= limit
        {
            break;
        }
        std::thread::sleep(Duration::from_secs(args.interval.max(1)));
    }
    Ok(())
}
