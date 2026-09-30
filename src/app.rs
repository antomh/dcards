//! The eframe application: top-level state, event pump and (for now) a stub UI.

use crossbeam_channel::Receiver;
use eframe::egui;

use crate::config::{Config, Theme};
use crate::events::{AppEvent, EventSender, Repaint};
use crate::paths::Paths;
use crate::single_instance::SingleInstance;
use crate::{hotkey, notify, tray};

/// How many recent events to keep for the stub UI.
const MAX_RECENT_EVENTS: usize = 50;

/// Root application object.
pub struct DcardsApp {
    config: Config,
    paths: Paths,
    rx: Receiver<AppEvent>,
    wayland: bool,
    quit: bool,
    recent_events: Vec<String>,
    // Kept alive for the lifetime of the application; dropped on shutdown.
    _tray: Option<tray::TrayGuard>,
    _hotkey: Option<hotkey::HotkeyRegistration>,
    _instance: Option<SingleInstance>,
    // Used by the HTTP layer in later stages.
    _runtime: tokio::runtime::Runtime,
}

impl DcardsApp {
    /// Build the application, wiring up the tray and global hotkey.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        paths: Paths,
        tx: EventSender,
        rx: Receiver<AppEvent>,
        repaint: Repaint,
        runtime: tokio::runtime::Runtime,
        instance: Option<SingleInstance>,
        wayland: bool,
    ) -> DcardsApp {
        repaint.set(cc.egui_ctx.clone());
        apply_theme(&cc.egui_ctx, config.general.theme);

        let tray = match tray::init(tx.clone(), repaint.clone()) {
            Ok(guard) => {
                tracing::info!("tray icon created");
                Some(guard)
            }
            Err(err) => {
                tracing::warn!(error = %err, "tray icon unavailable");
                None
            }
        };

        let hotkey = if wayland {
            tracing::info!("global hotkey disabled on Wayland");
            None
        } else {
            match hotkey::register(tx, repaint.clone()) {
                Ok(guard) => {
                    tracing::info!("registered global hotkey Ctrl+Alt+S");
                    Some(guard)
                }
                Err(err) => {
                    // No notification: the action is still available from the
                    // card draft in later stages.
                    tracing::warn!(error = %err, "failed to register global hotkey Ctrl+Alt+S");
                    None
                }
            }
        };

        DcardsApp {
            config,
            paths,
            rx,
            wayland,
            quit: false,
            recent_events: Vec::new(),
            _tray: tray,
            _hotkey: hotkey,
            _instance: instance,
            _runtime: runtime,
        }
    }

    fn handle_event(&mut self, ctx: &egui::Context, event: AppEvent) {
        tracing::debug!(?event, "app event");
        match event {
            AppEvent::Hotkey => {
                tracing::info!("global hotkey pressed");
                self.push_event("hotkey pressed");
            }
            AppEvent::Activate | AppEvent::TrayOpen => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.push_event("window activated");
            }
            AppEvent::TrayNewCard => {
                self.push_event("tray: new card (not implemented yet)");
            }
            AppEvent::TrayQuit => {
                self.push_event("tray: quit");
                self.quit = true;
            }
        }
    }

    fn push_event(&mut self, message: impl Into<String>) {
        self.recent_events.push(message.into());
        if self.recent_events.len() > MAX_RECENT_EVENTS {
            self.recent_events.remove(0);
        }
    }
}

impl eframe::App for DcardsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.rx.try_recv() {
            self.handle_event(ctx, event);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("dcards");
            ui.label("Stage 0 skeleton. Persistence, LLM and the card UI arrive in later stages.");
            ui.add_space(8.0);

            ui.label(format!("config: {}", self.paths.config_file.display()));
            ui.label(format!("database: {}", self.paths.db_file.display()));
            ui.label(format!("logs: {}", self.paths.log_dir.display()));
            ui.label(format!(
                "language pair: {}",
                language_pair_label(self.config.general.language_pair)
            ));

            if self.wayland {
                ui.add_space(4.0);
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "Wayland session detected: the global hotkey is unavailable.",
                );
            }

            ui.add_space(8.0);
            if ui.button("Test notification").clicked() {
                notify::info("dcards", "Notifications are working.");
            }

            ui.separator();
            ui.label("Recent events:");
            for line in self.recent_events.iter().rev().take(10) {
                ui.monospace(line);
            }
        });

        if self.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn apply_theme(ctx: &egui::Context, theme: Theme) {
    ctx.set_visuals(match theme {
        Theme::Dark => egui::Visuals::dark(),
        Theme::Light => egui::Visuals::light(),
    });
}

fn language_pair_label(pair: crate::config::LanguagePair) -> &'static str {
    use crate::config::LanguagePair;
    match pair {
        LanguagePair::EnEn => "en-en",
        LanguagePair::EnRu => "en-ru",
        LanguagePair::RuEn => "ru-en",
    }
}
