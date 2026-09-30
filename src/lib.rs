//! dcards — a flashcard dictionary for Linux.
//!
//! This library crate holds every module so that integration tests in `tests/`
//! can exercise them. The binary (`src/main.rs`) is a thin bootstrap wrapper.

pub mod app;
pub mod config;
pub mod db;
pub mod events;
pub mod hotkey;
pub mod llm;
pub mod logging;
pub mod notify;
pub mod paths;
pub mod pipeline;
pub mod selection;
pub mod single_instance;
pub mod tray;
pub mod ui;
pub mod validation;
