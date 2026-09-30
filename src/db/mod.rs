//! SQLite persistence layer.
//!
//! The database lives at `~/.local/share/dcards/dcards.db` (overridable via
//! configuration). Timestamps are unix seconds in UTC.
//!
//! Connections are created with WAL journaling, foreign keys enabled, a busy
//! timeout and a progress handler that aborts runaway statements. Every public
//! operation is wrapped in [`timed`], which logs slow queries and arms that
//! hard limit.

pub mod cards;
pub mod groups;
pub mod models;

use std::cell::Cell;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rusqlite::Connection;

pub use models::{Card, CardFilter, ExportRow, Group, Lang, LangPair};

/// Errors returned by the database layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A SQLite error.
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// A filesystem error while opening the database.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The requested group does not exist.
    #[error("group {0} was not found")]
    GroupNotFound(i64),
    /// The group still contains cards and may not be deleted.
    #[error("cannot delete group {0}: it still contains cards")]
    GroupNotEmpty(i64),
    /// Refusing to delete the only remaining group.
    #[error("cannot delete the last remaining group")]
    LastGroup,
    /// Group names must not be empty.
    #[error("group name must not be empty")]
    EmptyGroupName,
    /// A group with this name already exists.
    #[error("a group named {0:?} already exists")]
    DuplicateGroupName(String),
    /// The requested card does not exist.
    #[error("card {0} was not found")]
    CardNotFound(i64),
}

/// Convenience alias for database results.
pub type Result<T> = std::result::Result<T, DbError>;

/// Queries slower than this (ms) are logged as warnings.
const DEFAULT_SLOW_QUERY_WARN_MS: u64 = 50;
/// Hard limit (ms) after which a running statement is aborted.
const HARD_QUERY_LIMIT_MS: u64 = 2_000;
/// How often SQLite invokes the progress handler, in VM instructions.
const PROGRESS_OPS: i32 = 1_000;

static SLOW_QUERY_WARN_MS: AtomicU64 = AtomicU64::new(DEFAULT_SLOW_QUERY_WARN_MS);

thread_local! {
    /// Per-thread deadline of the currently running [`timed`] operation.
    static DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Override the slow-query warning threshold (from `config.logging`).
pub fn set_slow_query_warn_ms(ms: u64) {
    SLOW_QUERY_WARN_MS.store(ms, Ordering::Relaxed);
}

/// Run a database operation, logging it if it exceeds the warning threshold.
///
/// Also arms the per-thread hard limit checked by the connection's progress
/// handler; the previous deadline (for nested calls) is restored afterwards.
pub fn timed<T>(name: &str, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let start = Instant::now();
    let deadline = start + Duration::from_millis(HARD_QUERY_LIMIT_MS);
    let previous = DEADLINE.with(|slot| slot.replace(Some(deadline)));

    let result = f();

    DEADLINE.with(|slot| slot.set(previous));

    let elapsed_ms = start.elapsed().as_millis() as u64;
    if elapsed_ms > SLOW_QUERY_WARN_MS.load(Ordering::Relaxed) {
        tracing::warn!(query = name, elapsed_ms, "slow database query");
    }
    result
}

/// Progress handler installed on every connection: aborts the statement when
/// the current [`timed`] deadline has expired.
fn progress_callback() -> bool {
    let expired =
        DEADLINE.with(|slot| matches!(slot.get(), Some(deadline) if Instant::now() >= deadline));
    if expired {
        tracing::warn!("database statement exceeded the hard time limit; aborting");
    }
    expired
}

/// Open (creating if necessary) the database and apply connection pragmas.
///
/// The schema is created separately by [`migrate`].
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_millis(5_000))?;
    conn.progress_handler(PROGRESS_OPS, Some(progress_callback))?;
    Ok(conn)
}

/// Create the schema when it is missing. Idempotent; the version is tracked in
/// `PRAGMA user_version`.
pub fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cards (
    id         INTEGER PRIMARY KEY,
    front      TEXT    NOT NULL,
    back       TEXT    NOT NULL DEFAULT '',
    group_id   INTEGER NOT NULL REFERENCES groups(id) ON DELETE RESTRICT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cards_group   ON cards(group_id);
CREATE INDEX IF NOT EXISTS idx_cards_created ON cards(created_at);
"#;

/// Insert `name` as the default group when the table is empty.
pub fn bootstrap_default_group(conn: &Connection, name: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO groups (name, created_at)
         SELECT ?1, ?2 WHERE NOT EXISTS (SELECT 1 FROM groups)",
        rusqlite::params![name, now()],
    )?;
    Ok(())
}

/// Current unix time in seconds (UTC).
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Map a UNIQUE constraint violation to [`DbError::DuplicateGroupName`].
pub(crate) fn duplicate_name_error(error: DbError, name: &str) -> DbError {
    match error {
        DbError::Sqlite(rusqlite::Error::SqliteFailure(err, _))
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            DbError::DuplicateGroupName(name.to_string())
        }
        other => other,
    }
}
