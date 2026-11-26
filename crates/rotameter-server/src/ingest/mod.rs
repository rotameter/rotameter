#![forbid(unsafe_code)]
//! Bounded asynchronous MPSC ingestion ring buffer and background batch flusher.

pub(crate) mod processor;
pub(crate) mod types;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rotameter_core::storage::AnalyticsDataPlane;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

use crate::config::ServerConfig;
use crate::error::ApiError;

use processor::IngestProcessor;
use types::QueuedEvent;

enum IngestCmd {
    Event(Box<QueuedEvent>),
    Flush(oneshot::Sender<()>),
    Shutdown,
}

/// Bounded in-memory event queue driving batched [`AnalyticsDataPlane::write_batch`] commits.
#[derive(Clone)]
pub struct IngestPipeline {
    tx: mpsc::Sender<IngestCmd>,
    capacity: usize,
    depth: Arc<AtomicUsize>,
    worker: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl IngestPipeline {
    pub(crate) fn spawn(analytics: Arc<dyn AnalyticsDataPlane>, config: &ServerConfig) -> Self {
        let capacity = config.queue_capacity.max(1);
        let (tx, rx) = mpsc::channel(capacity);
        let depth = Arc::new(AtomicUsize::new(0));
        let handle = tokio::spawn(run_flusher(
            rx,
            analytics,
            depth.clone(),
            Duration::from_millis(config.flush_interval_ms.max(10)),
            config.max_batch_size.max(1),
        ));
        Self {
            tx,
            capacity,
            depth,
            worker: Arc::new(tokio::sync::Mutex::new(Some(handle))),
        }
    }

    pub(crate) fn disabled() -> Self {
        let (tx, _rx) = mpsc::channel(1);
        Self {
            tx,
            capacity: 0,
            depth: Arc::new(AtomicUsize::new(0)),
            worker: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub(crate) fn try_enqueue(&self, event: QueuedEvent) -> Result<(), ApiError> {
        self.depth.fetch_add(1, Ordering::Relaxed);
        if let Ok(()) = self.tx.try_send(IngestCmd::Event(Box::new(event))) {
            Ok(())
        } else {
            self.depth.fetch_sub(1, Ordering::Relaxed);
            Err(ApiError::service_unavailable(
                "queue_full",
                "ingestion buffer is full; retry shortly",
            ))
        }
    }

    /// Returns the approximate number of buffered events awaiting flush.
    pub fn queue_depth(&self) -> usize {
        self.depth.load(Ordering::Relaxed)
    }

    /// Returns the configured ring-buffer capacity.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Forces any currently queued events to be committed before returning (useful in tests).
    pub async fn flush_now(&self) {
        let (ack_tx, ack_rx) = oneshot::channel();
        if self.tx.send(IngestCmd::Flush(ack_tx)).await.is_ok() {
            let _ = ack_rx.await;
        }
    }

    /// Signals the background flusher to drain all pending batches within `timeout`.
    pub async fn shutdown(&self, timeout: Duration) {
        let _ = self.tx.send(IngestCmd::Shutdown).await;
        let handle = { self.worker.lock().await.take() };
        if let Some(join) = handle {
            let _ = tokio::time::timeout(timeout, join).await;
        }
    }
}

async fn run_flusher(
    mut rx: mpsc::Receiver<IngestCmd>,
    analytics: Arc<dyn AnalyticsDataPlane>,
    depth: Arc<AtomicUsize>,
    flush_every: Duration,
    max_batch: usize,
) {
    let mut processor = IngestProcessor::new();
    let mut buf = Vec::with_capacity(max_batch.min(512));
    let mut interval = tokio::time::interval(flush_every);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                match cmd {
                    Some(IngestCmd::Event(ev)) => {
                        buf.push(*ev);
                        if buf.len() >= max_batch {
                            commit_buf(&analytics, &mut processor, &depth, &mut buf).await;
                        }
                    }
                    Some(IngestCmd::Flush(ack)) => {
                        let mut acks = vec![ack];
                        drain_ready(&mut rx, &mut buf, &mut acks, usize::MAX);
                        commit_buf(&analytics, &mut processor, &depth, &mut buf).await;
                        for sender in acks {
                            let _ = sender.send(());
                        }
                    }
                    Some(IngestCmd::Shutdown) | None => {
                        let mut acks = Vec::new();
                        drain_ready(&mut rx, &mut buf, &mut acks, usize::MAX);
                        commit_buf(&analytics, &mut processor, &depth, &mut buf).await;
                        for sender in acks {
                            let _ = sender.send(());
                        }
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                if !buf.is_empty() {
                    commit_buf(&analytics, &mut processor, &depth, &mut buf).await;
                }
            }
        }
    }
}

fn drain_ready(
    rx: &mut mpsc::Receiver<IngestCmd>,
    buf: &mut Vec<QueuedEvent>,
    acks: &mut Vec<oneshot::Sender<()>>,
    limit: usize,
) {
    while buf.len() < limit {
        match rx.try_recv() {
            Ok(IngestCmd::Event(ev)) => buf.push(*ev),
            Ok(IngestCmd::Flush(ack)) => acks.push(ack),
            Ok(IngestCmd::Shutdown) | Err(_) => break,
        }
    }
}

async fn commit_buf(
    analytics: &Arc<dyn AnalyticsDataPlane>,
    processor: &mut IngestProcessor,
    depth: &Arc<AtomicUsize>,
    buf: &mut Vec<QueuedEvent>,
) {
    if buf.is_empty() {
        return;
    }
    let batch = std::mem::take(buf);
    let count = batch.len();
    let res = processor.process_batch(analytics.as_ref(), batch).await;
    depth.fetch_sub(count, Ordering::Relaxed);
    if let Err(err) = res {
        tracing::warn!(batch_len = count, "ingest batch commit failed: {err}");
    }
}
