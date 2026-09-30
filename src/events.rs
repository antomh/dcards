//! Cross-thread events delivered to the UI, and a repaint helper.

use std::sync::{Arc, OnceLock};

use crossbeam_channel::Sender;

/// An event produced by a background integration (tray, hotkey, second
/// instance) and consumed by the eframe update loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    /// The global hotkey was pressed.
    Hotkey,
    /// Another instance asked this one to come to the foreground.
    Activate,
    /// "Open dcards" was chosen in the tray menu (or the icon was clicked).
    TrayOpen,
    /// "New card" was chosen in the tray menu.
    TrayNewCard,
    /// "Quit" was chosen in the tray menu.
    TrayQuit,
}

/// Sender half of the UI event channel.
pub type EventSender = Sender<AppEvent>;

/// A cheap, cloneable handle that wakes the eframe event loop.
///
/// Background threads cannot schedule a repaint without the egui context, which
/// only exists once eframe has started, so the context is stored here lazily.
#[derive(Clone, Default)]
pub struct Repaint(Arc<OnceLock<egui::Context>>);

impl Repaint {
    /// Store the egui context. Called once, from the app constructor.
    pub fn set(&self, ctx: egui::Context) {
        let _ = self.0.set(ctx);
    }

    /// Request a repaint, if the context is already available.
    pub fn notify(&self) {
        if let Some(ctx) = self.0.get() {
            ctx.request_repaint();
        }
    }
}
