#![forbid(unsafe_code)]
//! Log-file `--follow` tailer with inode/truncation rotation detection and `SIGTERM` hook.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::exit::CliError;
use crate::import_stream::ImportChannel;

pub(crate) fn stream_follow_file(
    path: &Path,
    channel: &ImportChannel,
    shutdown: &AtomicBool,
) -> Result<(), CliError> {
    let mut reader =
        BufReader::new(File::open(path).map_err(|err| {
            CliError::runtime(format!("failed to open `{}`: {err}", path.display()))
        })?);
    let mut pos = 0_u64;
    let mut inode = file_inode(path);
    let mut line = String::new();

    while !shutdown.load(Ordering::Relaxed) {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                std::thread::sleep(Duration::from_millis(80));
                if should_reopen(path, inode, pos)
                    && let Ok(mut file) = File::open(path)
                {
                    let _ = file.seek(SeekFrom::Start(0));
                    reader = BufReader::new(file);
                    pos = 0;
                    inode = file_inode(path);
                }
            }
            Ok(n) => {
                pos = pos.saturating_add(u64::try_from(n).unwrap_or(0));
                channel.push_line(&line);
            }
            Err(err) => {
                return Err(CliError::runtime(format!(
                    "failed reading `{}`: {err}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn should_reopen(path: &Path, prev_inode: Option<u64>, pos: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let rotated = prev_inode.is_some() && file_inode_meta(&meta) != prev_inode;
    rotated || meta.len() < pos
}

#[cfg(unix)]
fn file_inode(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| file_inode_meta(&m))
}

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn file_inode_meta(meta: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.ino())
}

#[cfg(not(unix))]
fn file_inode(_path: &Path) -> Option<u64> {
    None
}

#[cfg(not(unix))]
fn file_inode_meta(_meta: &std::fs::Metadata) -> Option<u64> {
    None
}

#[cfg(all(unix, feature = "server"))]
pub(crate) fn install_shutdown_hook(shutdown: Arc<AtomicBool>) {
    let _ = std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        rt.block_on(async move {
            let ctrl_c = tokio::signal::ctrl_c();
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
            match sigterm {
                Some(ref mut term) => {
                    tokio::select! {
                        _ = ctrl_c => {}
                        _ = term.recv() => {}
                    }
                }
                None => {
                    let _ = ctrl_c.await;
                }
            }
            shutdown.store(true, Ordering::SeqCst);
        });
    });
}

#[cfg(not(all(unix, feature = "server")))]
pub(crate) fn install_shutdown_hook(_shutdown: Arc<AtomicBool>) {}
