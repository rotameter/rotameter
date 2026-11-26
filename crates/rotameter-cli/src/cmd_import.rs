#![forbid(unsafe_code)]
//! `rotameter import` command handler (one-shot file, `--stdin`/`-`, `--unix-socket`, `--follow`).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rotameter_client::types::SiteRef;

use crate::cli::GlobalOpts;
use crate::cli_sub::{ImportArgs, ImportFormat};
use crate::exit::CliError;
use crate::import_file::{print_import_summary, run_dry_run_file, run_oneshot_file};
use crate::import_follow::{install_shutdown_hook, stream_follow_file};
use crate::import_socket::{resolve_socket_mode, stream_unix_socket};
use crate::import_stream::{ImportChannel, run_streaming_pipeline};

pub(crate) fn run_import(global: &GlobalOpts, args: &ImportArgs) -> Result<(), CliError> {
    let site_ref = SiteRef::parse(&args.site)?;
    let source = resolve_source(args)?;
    let is_streaming = !matches!(source, ImportSource::File(_));
    let apply_proxy_filter =
        !args.no_proxy_filter && (is_streaming || args.input_format != ImportFormat::Jsonl);

    if args.dry_run {
        return match source {
            ImportSource::File(p) => {
                run_dry_run_file(global, args, &site_ref, Some(p), apply_proxy_filter)
            }
            ImportSource::Stdin => {
                run_dry_run_file(global, args, &site_ref, None, apply_proxy_filter)
            }
            ImportSource::Follow(_) | ImportSource::UnixSocket(_) => Err(CliError::usage(
                "`--dry-run` is not supported with `--follow` or `--unix-socket`",
            )),
        };
    }

    match source {
        ImportSource::File(path) => {
            run_oneshot_file(global, args, &site_ref, path, apply_proxy_filter)
        }
        ImportSource::Stdin => {
            run_streaming(global, args, &site_ref, apply_proxy_filter, |ch, stop| {
                let stdin = std::io::stdin();
                for line_res in BufReader::new(stdin.lock()).lines() {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let line = line_res
                        .map_err(|err| CliError::runtime(format!("failed reading stdin: {err}")))?;
                    ch.push_line(&line);
                }
                Ok(())
            })
        }
        ImportSource::Follow(path) => {
            run_streaming(global, args, &site_ref, apply_proxy_filter, |ch, stop| {
                stream_follow_file(path, ch, stop)
            })
        }
        ImportSource::UnixSocket(path) => {
            let mode = resolve_socket_mode(args.socket_mode, args.input_format);
            run_streaming(global, args, &site_ref, apply_proxy_filter, |ch, stop| {
                let ch_cb = Arc::clone(ch);
                let cb = Arc::new(move |line: &str| ch_cb.push_line(line));
                stream_unix_socket(path, mode, stop, &cb)
            })
        }
    }
}

enum ImportSource<'a> {
    File(&'a Path),
    Follow(&'a Path),
    Stdin,
    UnixSocket(&'a Path),
}

fn resolve_source(args: &ImportArgs) -> Result<ImportSource<'_>, CliError> {
    let file_is_dash = args.file.as_deref() == Some(Path::new("-"));
    let use_stdin = args.stdin || file_is_dash;
    let has_socket = args.unix_socket.is_some();
    let has_path = args.file.as_deref().is_some_and(|p| p != Path::new("-"));

    let count = usize::from(use_stdin) + usize::from(has_socket) + usize::from(has_path);
    if count == 0 {
        return Err(CliError::usage(
            "provide an input `<file>`, `-`, `--stdin`, or `--unix-socket <path>`",
        ));
    }
    if count > 1 {
        return Err(CliError::usage(
            "input sources (`<file>`, `--stdin`, `--unix-socket`) are mutually exclusive",
        ));
    }
    if args.follow && !has_path {
        return Err(CliError::usage("`--follow` requires a log `<file>` path"));
    }

    if use_stdin {
        Ok(ImportSource::Stdin)
    } else if let Some(sock) = args.unix_socket.as_deref() {
        Ok(ImportSource::UnixSocket(sock))
    } else if let Some(path) = args.file.as_deref() {
        if args.follow {
            Ok(ImportSource::Follow(path))
        } else {
            Ok(ImportSource::File(path))
        }
    } else {
        Err(CliError::usage("missing import source"))
    }
}

fn run_streaming<F>(
    global: &GlobalOpts,
    args: &ImportArgs,
    site_ref: &SiteRef,
    apply_proxy_filter: bool,
    producer: F,
) -> Result<(), CliError>
where
    F: FnOnce(&Arc<ImportChannel>, &Arc<AtomicBool>) -> Result<(), CliError>,
{
    let shutdown = Arc::new(AtomicBool::new(false));
    install_shutdown_hook(Arc::clone(&shutdown));
    let stats = run_streaming_pipeline(
        global,
        site_ref,
        args.input_format,
        apply_proxy_filter,
        args.batch_size,
        args.flush_interval_ms,
        args.max_queue,
        &shutdown,
        producer,
    )?;
    print_import_summary(
        global,
        site_ref,
        stats.parsed,
        stats.skipped,
        stats.accepted,
        stats.rejected,
        stats.dropped_backpressure,
        stats.dropped_upstream,
    )
}
