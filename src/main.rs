//! dcards — a flashcard dictionary for Linux.
//!
//! Process bootstrap: paths, configuration, logging, single instance, database
//! and the eframe application.

use std::sync::Mutex;

use crossbeam_channel::unbounded;
use eframe::egui;

use dcards::app::DcardsApp;
use dcards::config::Config;
use dcards::db;
use dcards::events::Repaint;
use dcards::paths::Paths;
use dcards::{hotkey, logging, single_instance};

fn main() -> anyhow::Result<()> {
    let paths = Paths::discover()?;
    paths.ensure_dirs()?;

    let config = match Config::load(&paths.config_file) {
        Ok(config) => config,
        Err(err) => {
            eprintln!(
                "dcards: failed to load {} ({err:#}); using defaults",
                paths.config_file.display()
            );
            Config::default()
        }
    };

    // Keep the log guard alive for the whole process.
    let _log_guard = logging::init(
        &paths.log_dir,
        &config.logging.level,
        config.logging.max_files,
    )?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "dcards starting");

    let (tx, rx) = unbounded();
    let repaint = Repaint::default();

    let instance = match single_instance::acquire(&paths.socket_file, tx.clone(), repaint.clone()) {
        Ok(Some(guard)) => Some(guard),
        Ok(None) => {
            println!("dcards: another instance is already running; requested activation");
            return Ok(());
        }
        Err(err) => {
            tracing::warn!(error = %err, "single-instance guard unavailable");
            None
        }
    };

    let wayland = hotkey::is_wayland();
    if wayland {
        tracing::warn!("Wayland session detected: global hotkey will be disabled");
    }

    // Open the database, create the schema and make sure a default group exists.
    let db_path = config
        .storage
        .db_path
        .clone()
        .unwrap_or_else(|| paths.db_file.clone());
    let connection = db::open(&db_path)?;
    db::migrate(&connection)?;
    db::bootstrap_default_group(&connection, &config.general.default_group)?;
    db::set_slow_query_warn_ms(config.logging.slow_query_warn_ms);
    tracing::info!(path = %db_path.display(), "database ready");
    let db = Mutex::new(connection);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("dcards")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };

    let run_result = eframe::run_native(
        "dcards",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(DcardsApp::new(
                cc, config, paths, db, tx, rx, repaint, runtime, instance, wayland,
            )))
        }),
    );
    if let Err(err) = run_result {
        // `eframe::Error` is not `Send + Sync`, so map it manually.
        anyhow::bail!("eframe exited with an error: {err}");
    }

    tracing::info!("dcards stopped");
    Ok(())
}
