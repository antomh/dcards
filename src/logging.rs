//! Structured logging with daily rotation.
//!
//! Logs are written to `~/.local/state/dcards/logs/dcards.<date>.log` and the
//! number of retained files is bounded by [`crate::config::LoggingConfig`].
//!
//! # Secret handling
//!
//! The LLM API key **must never** be passed to any `tracing` macro, embedded in
//! an error message, or otherwise written to the log. [`crate::config::LlmConfig`]
//! implements a redacting `Debug` for exactly this reason; keep it that way.

use std::path::Path;

use anyhow::Context;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Keeps the non-blocking log writer alive; dropping it flushes pending lines.
#[must_use = "the log guard must be kept alive until shutdown"]
pub struct LogGuard {
    _guard: WorkerGuard,
}

/// Initialise the global tracing subscriber.
///
/// Falls back to `info` when `level` is not a valid `EnvFilter` directive.
/// Returns an error if the log directory cannot be written to.
pub fn init(log_dir: &Path, level: &str, max_files: usize) -> anyhow::Result<LogGuard> {
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log directory {}", log_dir.display()))?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("dcards")
        .filename_suffix("log")
        .max_log_files(max_files.max(1))
        .build(log_dir)
        .with_context(|| format!("failed to open log file in {}", log_dir.display()))?;

    let (writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .try_init()
        .ok();

    Ok(LogGuard { _guard: guard })
}
