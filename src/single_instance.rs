//! Single-instance enforcement over a Unix domain socket.
//!
//! The first process binds `$XDG_RUNTIME_DIR/dcards.sock`; a second one
//! connects to it, sends `activate`, and exits so the running process can raise
//! its window. Stale sockets from crashed processes are detected and removed.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use crate::events::{AppEvent, EventSender, Repaint};

/// Held by the primary instance; removes the socket on drop.
pub struct SingleInstance {
    socket: PathBuf,
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
    }
}

/// Try to become the primary instance.
///
/// * `Ok(Some(_))` — this process owns the socket; keep the guard alive.
/// * `Ok(None)` — another instance is running; it was asked to activate and the
///   caller should exit.
/// * `Err(_)` — the socket could not be used; the caller may continue without
///   single-instance protection.
pub fn acquire(
    socket: &Path,
    tx: EventSender,
    repaint: Repaint,
) -> anyhow::Result<Option<SingleInstance>> {
    if socket.exists() {
        match UnixStream::connect(socket) {
            Ok(mut stream) => {
                let _ = stream.write_all(b"activate\n");
                return Ok(None);
            }
            // Stale socket left behind by a crashed process.
            Err(_) => {
                let _ = fs::remove_file(socket);
            }
        }
    }

    let listener = UnixListener::bind(socket)?;
    let socket = socket.to_path_buf();

    std::thread::Builder::new()
        .name("dcards-single-instance".to_string())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 64];
                let _ = stream.read(&mut buf);
                let _ = tx.send(AppEvent::Activate);
                repaint.notify();
            }
        })?;

    Ok(Some(SingleInstance { socket }))
}
