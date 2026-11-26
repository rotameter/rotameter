#![forbid(unsafe_code)]
//! Unix domain socket receiver (`SOCK_DGRAM` syslog and `SOCK_STREAM` NDJSON) for `rotameter import`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::cli_sub::{ImportFormat, SocketMode};
use crate::exit::CliError;

pub(crate) const fn resolve_socket_mode(mode: SocketMode, format: ImportFormat) -> SocketMode {
    match mode {
        SocketMode::Auto => match format {
            ImportFormat::CaddyJson | ImportFormat::Jsonl => SocketMode::Stream,
            ImportFormat::Auto | ImportFormat::Clf => SocketMode::Dgram,
        },
        explicit => explicit,
    }
}

#[cfg(unix)]
pub(crate) fn stream_unix_socket<F>(
    path: &Path,
    mode: SocketMode,
    shutdown: &Arc<AtomicBool>,
    on_line: &Arc<F>,
) -> Result<(), CliError>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            CliError::runtime(format!(
                "failed to create socket parent `{}`: {err}",
                parent.display()
            ))
        })?;
    }

    let res = match mode {
        SocketMode::Dgram | SocketMode::Auto => run_dgram(path, shutdown, on_line.as_ref()),
        SocketMode::Stream => run_stream(path, shutdown, on_line),
    };
    let _ = std::fs::remove_file(path);
    res
}

#[cfg(not(unix))]
pub(crate) fn stream_unix_socket<F>(
    _path: &Path,
    _mode: SocketMode,
    _shutdown: &Arc<AtomicBool>,
    _on_line: &Arc<F>,
) -> Result<(), CliError>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    Err(CliError::usage(
        "`--unix-socket` is only supported on Unix platforms",
    ))
}

#[cfg(unix)]
fn run_dgram<F>(path: &Path, shutdown: &AtomicBool, on_line: &F) -> Result<(), CliError>
where
    F: Fn(&str) + Send + Sync,
{
    use std::io::ErrorKind;
    use std::os::unix::net::UnixDatagram;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let sock = UnixDatagram::bind(path).map_err(|err| {
        CliError::runtime(format!(
            "failed to bind Unix datagram socket `{}`: {err}",
            path.display()
        ))
    })?;
    sock.set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|err| CliError::runtime(format!("failed to set socket timeout: {err}")))?;

    let mut buf = vec![0_u8; 65_536];
    while !shutdown.load(Ordering::Relaxed) {
        match sock.recv(&mut buf) {
            Ok(n) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                for line in text.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        on_line(trimmed);
                    }
                }
            }
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) => {
                return Err(CliError::runtime(format!(
                    "Unix datagram receive failed on `{}`: {err}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn run_stream<F>(path: &Path, shutdown: &Arc<AtomicBool>, on_line: &Arc<F>) -> Result<(), CliError>
where
    F: Fn(&str) + Send + Sync + 'static,
{
    use std::io::{BufRead, BufReader, ErrorKind};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    let listener = UnixListener::bind(path).map_err(|err| {
        CliError::runtime(format!(
            "failed to bind Unix stream socket `{}`: {err}",
            path.display()
        ))
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|err| CliError::runtime(format!("failed to set listener nonblocking: {err}")))?;

    let mut workers = Vec::new();
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(150)));
                let stop = Arc::clone(shutdown);
                let cb = Arc::clone(on_line);
                workers.push(std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    while !stop.load(Ordering::Relaxed) {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => break,
                            Ok(_) => {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    cb(trimmed);
                                }
                            }
                            Err(err)
                                if matches!(
                                    err.kind(),
                                    ErrorKind::WouldBlock
                                        | ErrorKind::TimedOut
                                        | ErrorKind::Interrupted
                                ) => {}
                            Err(_) => break,
                        }
                    }
                }));
                workers.retain(|h| !h.is_finished());
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(err) if err.kind() == ErrorKind::Interrupted => {}
            Err(err) => {
                return Err(CliError::runtime(format!(
                    "Unix stream accept failed on `{}`: {err}",
                    path.display()
                )));
            }
        }
    }
    for handle in workers {
        let _ = handle.join();
    }
    Ok(())
}
