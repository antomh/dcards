//! Filesystem locations for dcards.
//!
//! All user-visible files are derived from the XDG base directories via
//! [`directories::ProjectDirs`]:
//!
//! * config: `~/.config/dcards/config.toml`
//! * data:   `~/.local/share/dcards/dcards.db`
//! * state:  `~/.local/state/dcards/logs/`
//!
//! The single-instance socket lives directly in `$XDG_RUNTIME_DIR` (falling
//! back to `$TMPDIR`/`/tmp` when it is unset).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

/// Permission bits used for files that may contain the API key.
pub const PRIVATE_FILE_MODE: u32 = 0o600;

/// Resolved dcards paths.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `~/.config/dcards`.
    pub config_dir: PathBuf,
    /// `~/.config/dcards/config.toml`.
    pub config_file: PathBuf,
    /// `~/.local/share/dcards`.
    pub data_dir: PathBuf,
    /// `~/.local/share/dcards/dcards.db`.
    pub db_file: PathBuf,
    /// `~/.local/state/dcards`.
    pub state_dir: PathBuf,
    /// `~/.local/state/dcards/logs`.
    pub log_dir: PathBuf,
    /// Unix socket used to detect and activate an already running instance.
    pub socket_file: PathBuf,
}

impl Paths {
    /// Discover the standard paths for the current user.
    ///
    /// Returns an error only when the platform has no home directory, which is
    /// a hard failure for a desktop application.
    pub fn discover() -> anyhow::Result<Paths> {
        let dirs = ProjectDirs::from("", "", "dcards")
            .ok_or_else(|| anyhow::anyhow!("could not determine the user home directory"))?;

        let config_dir = dirs.config_dir().to_path_buf();
        let data_dir = dirs.data_dir().to_path_buf();
        let state_dir = dirs.state_dir().unwrap_or(&config_dir).to_path_buf();

        Ok(Paths {
            config_file: config_dir.join("config.toml"),
            db_file: data_dir.join("dcards.db"),
            log_dir: state_dir.join("logs"),
            socket_file: runtime_socket_path(),
            config_dir,
            data_dir,
            state_dir,
        })
    }

    /// Create every directory dcards needs, if missing.
    pub fn ensure_dirs(&self) -> io::Result<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.state_dir,
            &self.log_dir,
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

/// Path of the single-instance socket.
///
/// Prefers `$XDG_RUNTIME_DIR`, falls back to `$TMPDIR`/`/tmp` with the user
/// name embedded so that different users do not collide.
fn runtime_socket_path() -> PathBuf {
    if let Some(dir) = non_empty_env("XDG_RUNTIME_DIR") {
        return Path::new(&dir).join("dcards.sock");
    }
    let dir = non_empty_env("TMPDIR").unwrap_or_else(|| "/tmp".to_string());
    let user = non_empty_env("USER").unwrap_or_else(|| "unknown".to_string());
    Path::new(&dir).join(format!("dcards-{user}.sock"))
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Create (or truncate) a file with `0600` permissions.
///
/// Used for `config.toml` and its temporary sibling so the API key is never
/// exposed to other users, even transiently.
pub fn create_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
}

/// Force `0600` on an existing file.
pub fn set_private(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
}
