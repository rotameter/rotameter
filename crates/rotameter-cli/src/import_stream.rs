#![forbid(unsafe_code)]
//! Bounded streaming queue and time+size batch flusher for `rotameter import`.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError};
use std::time::{Duration, Instant};

use rotameter_client::types::{BatchEventItem, BatchEventsRequest, MAX_BATCH_EVENTS, SiteRef};

use crate::cli::GlobalOpts;
use crate::cli_sub::ImportFormat;
use crate::context::{prefer_local_db, remote_client};
use crate::exit::CliError;
use crate::import_parse::{default_host_for_site, parse_import_line};

pub(crate) struct StreamStats {
    pub(crate) parsed: u64,
    pub(crate) skipped: u64,
    pub(crate) accepted: u64,
    pub(crate) rejected: u64,
    pub(crate) dropped_backpressure: u64,
    pub(crate) dropped_upstream: u64,
}

pub(crate) struct ImportChannel {
    tx: SyncSender<BatchEventItem>,
    site_ref: SiteRef,
    host: String,
    format: ImportFormat,
    apply_proxy_filter: bool,
    parsed: AtomicU64,
    skipped: AtomicU64,
    dropped_backpressure: AtomicU64,
}

impl ImportChannel {
    pub(crate) fn push_line(&self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        match parse_import_line(
            &self.site_ref,
            &self.host,
            trimmed,
            self.format,
            self.apply_proxy_filter,
        ) {
            Some(item) => {
                self.parsed.fetch_add(1, Ordering::Relaxed);
                if matches!(
                    self.tx.try_send(item),
                    Err(TrySendError::Full(_) | TrySendError::Disconnected(_))
                ) {
                    self.dropped_backpressure.fetch_add(1, Ordering::Relaxed);
                }
            }
            None => {
                self.skipped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_streaming_pipeline<F>(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    format: ImportFormat,
    apply_proxy_filter: bool,
    batch_size: usize,
    flush_interval_ms: u64,
    max_queue: usize,
    shutdown: &Arc<std::sync::atomic::AtomicBool>,
    producer: F,
) -> Result<StreamStats, CliError>
where
    F: FnOnce(&Arc<ImportChannel>, &Arc<std::sync::atomic::AtomicBool>) -> Result<(), CliError>,
{
    let batch_cap = batch_size.clamp(1, MAX_BATCH_EVENTS);
    let flush_every = Duration::from_millis(flush_interval_ms.max(20));
    let queue_cap = max_queue.max(1);
    let use_local = prefer_local_db(global);
    let client = (!use_local).then(|| remote_client(global)).transpose()?;
    let (tx, rx) = mpsc::sync_channel::<BatchEventItem>(queue_cap);
    let channel = Arc::new(ImportChannel {
        tx,
        site_ref: site_ref.clone(),
        host: default_host_for_site(site_ref),
        format,
        apply_proxy_filter,
        parsed: AtomicU64::new(0),
        skipped: AtomicU64::new(0),
        dropped_backpressure: AtomicU64::new(0),
    });

    let global_cloned = global.clone();
    let site_cloned = site_ref.clone();
    let consumer = std::thread::spawn(move || {
        flush_loop(
            &rx,
            &global_cloned,
            &site_cloned,
            client.as_ref(),
            use_local,
            batch_cap,
            flush_every,
        )
    });

    let prod_res = producer(&channel, shutdown);
    let parsed = channel.parsed.load(Ordering::Relaxed);
    let skipped = channel.skipped.load(Ordering::Relaxed);
    let dropped_backpressure = channel.dropped_backpressure.load(Ordering::Relaxed);
    drop(channel);

    let (accepted, rejected, dropped_upstream) = consumer
        .join()
        .map_err(|_| CliError::runtime("import flush worker panicked"))?;
    prod_res?;
    Ok(StreamStats {
        parsed,
        skipped,
        accepted,
        rejected,
        dropped_backpressure,
        dropped_upstream,
    })
}

fn flush_loop(
    rx: &mpsc::Receiver<BatchEventItem>,
    global: &GlobalOpts,
    site_ref: &SiteRef,
    client: Option<&rotameter_client::BlockingRotameterClient>,
    use_local: bool,
    batch_cap: usize,
    flush_every: Duration,
) -> (u64, u64, u64) {
    let mut buf = Vec::with_capacity(batch_cap);
    let mut last_flush = Instant::now();
    let mut totals = (0_u64, 0_u64, 0_u64);

    loop {
        let wait = flush_every
            .saturating_sub(last_flush.elapsed())
            .min(Duration::from_millis(50));
        match rx.recv_timeout(wait) {
            Ok(item) => {
                buf.push(item);
                if buf.len() >= batch_cap {
                    flush_buf(&mut buf, global, site_ref, client, use_local, &mut totals);
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if !buf.is_empty() && last_flush.elapsed() >= flush_every {
                    flush_buf(&mut buf, global, site_ref, client, use_local, &mut totals);
                    last_flush = Instant::now();
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if !buf.is_empty() {
                    flush_buf(&mut buf, global, site_ref, client, use_local, &mut totals);
                }
                break;
            }
        }
    }
    totals
}

fn flush_buf(
    buf: &mut Vec<BatchEventItem>,
    global: &GlobalOpts,
    site_ref: &SiteRef,
    client: Option<&rotameter_client::BlockingRotameterClient>,
    use_local: bool,
    totals: &mut (u64, u64, u64),
) {
    let chunk = std::mem::take(buf);
    let count = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
    let res = if use_local {
        ingest_local_chunk(global, site_ref, &chunk)
    } else if let Some(c) = client {
        c.post_events_batch(&BatchEventsRequest { events: chunk })
            .map(|r| (r.accepted, r.rejected))
            .map_err(CliError::from)
    } else {
        Err(CliError::runtime("missing import destination"))
    };
    match res {
        Ok((acc, rej)) => {
            totals.0 += acc;
            totals.1 += rej;
        }
        Err(err) => {
            totals.2 += count;
            eprintln!("rotameter import: upstream flush error (dropped {count}): {err}");
        }
    }
}

#[cfg(feature = "sqlite")]
fn ingest_local_chunk(
    global: &GlobalOpts,
    site_ref: &SiteRef,
    chunk: &[BatchEventItem],
) -> Result<(u64, u64), CliError> {
    crate::local_import::ingest_local(global, site_ref, chunk)
}

#[cfg(not(feature = "sqlite"))]
fn ingest_local_chunk(
    _global: &GlobalOpts,
    _site_ref: &SiteRef,
    _chunk: &[BatchEventItem],
) -> Result<(u64, u64), CliError> {
    Err(CliError::usage(
        "local `--db` import requires the `sqlite` feature",
    ))
}
