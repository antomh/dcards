//! The eframe application: top-level state, event pump and view routing.

use std::sync::Mutex;

use crossbeam_channel::Receiver;
use eframe::egui;
use rusqlite::Connection;

use crate::config::{Config, Theme};
use crate::db::{self, cards, groups, Card, CardFilter, Group, Lang, LangPair};
use crate::events::{AppEvent, EventSender, Repaint};
use crate::llm::{self, HttpLlmClient, LlmClient};
use crate::paths::Paths;
use crate::pipeline::{self, CleanOutcome, Draft, DraftOutcome, TranslationEffect};
use crate::single_instance::SingleInstance;
use crate::ui::widgets::DateFilterState;
use crate::ui::TestConnection;
use crate::ui::{CardEditor, GroupEditor, GroupEditorMode, IoDialog, SettingsForm, View};
use crate::{hotkey, notify, selection, tray, validation};

/// Root application object.
pub struct DcardsApp {
    pub(crate) config: Config,
    pub(crate) paths: Paths,
    pub(crate) db: Mutex<Connection>,
    pub(crate) tx: EventSender,
    pub(crate) rx: Receiver<AppEvent>,
    pub(crate) repaint: Repaint,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) wayland: bool,
    pub(crate) quit: bool,

    pub(crate) view: View,
    pub(crate) groups: Vec<Group>,
    pub(crate) selected_group: Option<i64>,
    pub(crate) cards: Vec<Card>,
    pub(crate) cards_total: i64,
    pub(crate) selected_card: Option<i64>,
    pub(crate) date_filter: DateFilterState,
    pub(crate) review: Option<crate::review::Session>,
    pub(crate) review_filter: DateFilterState,
    pub(crate) card_editor: Option<CardEditor>,
    pub(crate) io_dialog: Option<IoDialog>,
    pub(crate) draft: Option<Draft>,
    pub(crate) next_generation: u64,
    pub(crate) group_editor: Option<GroupEditor>,
    pub(crate) settings: SettingsForm,
    pub(crate) test_connection: TestConnection,
    pub(crate) status: Option<String>,

    // Kept alive for the lifetime of the application; dropped on shutdown.
    _tray: Option<tray::TrayGuard>,
    _hotkey: Option<hotkey::HotkeyRegistration>,
    _instance: Option<SingleInstance>,
}

impl DcardsApp {
    /// Build the application, wiring up the tray, hotkey and initial data.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        paths: Paths,
        db: Mutex<Connection>,
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
            match hotkey::register(tx.clone(), repaint.clone()) {
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

        let settings = SettingsForm::from_config(&config);
        let mut app = DcardsApp {
            config,
            paths,
            db,
            tx,
            rx,
            repaint,
            runtime,
            wayland,
            quit: false,
            view: View::Groups,
            groups: Vec::new(),
            selected_group: None,
            cards: Vec::new(),
            cards_total: 0,
            selected_card: None,
            date_filter: DateFilterState::default(),
            review: None,
            review_filter: DateFilterState::default(),
            card_editor: None,
            io_dialog: None,
            draft: None,
            next_generation: 0,
            group_editor: None,
            settings,
            test_connection: TestConnection::default(),
            status: None,
            _tray: tray,
            _hotkey: hotkey,
            _instance: instance,
        };
        app.refresh_groups();
        app
    }

    /// Lock the database, recovering from a poisoned mutex.
    pub(crate) fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> db::Result<T>,
    ) -> db::Result<T> {
        let conn = self
            .db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&conn)
    }

    /// Lock the database and run `f`, returning its value unchanged.
    pub(crate) fn with_conn_raw<T>(&self, f: impl FnOnce(&Connection) -> T) -> T {
        let conn = self
            .db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&conn)
    }

    /// The group currently selected in the main view.
    pub(crate) fn current_group(&self) -> Option<&Group> {
        let id = self.selected_group?;
        self.groups.iter().find(|group| group.id == id)
    }

    /// Reload the group list and keep a valid selection.
    pub(crate) fn refresh_groups(&mut self) {
        match self.with_conn(groups::list) {
            Ok(groups) => {
                self.groups = groups;

                if let Some(id) = self.selected_group {
                    if !self.groups.iter().any(|group| group.id == id) {
                        self.selected_group = None;
                    }
                }
                if self.selected_group.is_none() {
                    let default_name = self.config.general.default_group.clone();
                    self.selected_group = self
                        .groups
                        .iter()
                        .find(|group| group.name == default_name)
                        .map(|group| group.id)
                        .or_else(|| self.groups.first().map(|group| group.id));
                    self.selected_card = None;
                }
                self.refresh_cards();
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to load groups");
                self.set_error(format!("Failed to load groups: {err}"));
            }
        }
    }

    /// Reload the cards of the selected group using the current filter.
    pub(crate) fn refresh_cards(&mut self) {
        let Some(group_id) = self.selected_group else {
            self.cards.clear();
            self.cards_total = 0;
            return;
        };

        let filter = self.current_filter(group_id);
        let limit = self.config.general.cards_limit;
        let result = self.with_conn(|conn| {
            let cards = cards::list_by_group(conn, group_id, &filter, limit)?;
            let total = cards::count_by_group(conn, group_id)?;
            Ok((cards, total))
        });

        match result {
            Ok((cards, total)) => {
                self.cards = cards;
                self.cards_total = total;
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to load cards");
                self.set_error(format!("Failed to load cards: {err}"));
            }
        }
    }

    fn current_filter(&self, group_id: i64) -> CardFilter {
        let (from, to) = self
            .date_filter
            .resolve(chrono::Utc::now().timestamp(), local_offset_seconds());
        CardFilter {
            group_id: Some(group_id),
            from,
            to,
        }
    }

    /// Open a blank draft. No translation request is made.
    pub(crate) fn new_card(&mut self) {
        self.show_draft(String::new(), String::new(), None, None);
    }

    /// The default group id: the configured group, else the first by name.
    fn default_group_id(&self) -> Option<i64> {
        let name = self.config.general.default_group.as_str();
        self.groups
            .iter()
            .find(|group| group.name == name)
            .map(|group| group.id)
            .or_else(|| self.groups.first().map(|group| group.id))
    }

    /// Open or refresh the draft window.
    ///
    /// When `request` is `Some`, a translation is started and its result is
    /// delivered through the event channel, tagged with a new generation.
    fn show_draft(
        &mut self,
        front: String,
        back: String,
        note: Option<String>,
        request: Option<llm::LlmRequest>,
    ) {
        let kept_group = self
            .draft
            .as_ref()
            .map(|draft| draft.group_id)
            .filter(|id| self.groups.iter().any(|group| group.id == *id));
        let Some(group_id) = kept_group.or_else(|| self.default_group_id()) else {
            self.set_error("No groups available.");
            return;
        };

        let generation = if request.is_some() {
            self.next_generation = self.next_generation.wrapping_add(1);
            self.next_generation
        } else {
            self.next_generation
        };

        self.draft = Some(Draft {
            front,
            back,
            group_id,
            note,
            loading: request.is_some(),
            generation,
            focus_front: true,
        });

        if let Some(request) = request {
            let tx = self.tx.clone();
            let repaint = self.repaint.clone();
            self.runtime.spawn(async move {
                let result = match HttpLlmClient::new() {
                    Ok(client) => client.complete(request).await,
                    Err(err) => Err(err),
                };
                let event = match result {
                    Ok(back) => AppEvent::TranslationDone { generation, back },
                    Err(err) => AppEvent::TranslationFailed {
                        generation,
                        error: err.to_string(),
                    },
                };
                let _ = tx.send(event);
                repaint.notify();
            });
        }
    }

    /// Save the draft as a new card.
    pub(crate) fn save_draft(&mut self) {
        let Some(draft) = self.draft.as_ref() else {
            return;
        };
        let front = draft.front.trim().to_string();
        let back = draft.back.clone();
        let group_id = draft.group_id;

        if front.is_empty() {
            self.set_error("Front must not be empty");
            return;
        }

        match self.with_conn(|conn| pipeline::save_card(conn, &front, &back, group_id)) {
            Ok(card) => {
                self.draft = None;
                self.selected_card = Some(card.id);
                self.refresh_cards();
                self.set_status("Card saved");
            }
            Err(err) => {
                if let Some(draft) = &mut self.draft {
                    draft.note = Some(err.to_string());
                }
                self.set_error(format!("Failed to save card: {err}"));
            }
        }
    }

    /// Clean the front field and (re)translate it, keeping the current back.
    pub(crate) fn draft_clean_translate(&mut self) {
        let config = self.config.clone();
        let Some(draft) = self.draft.as_ref() else {
            return;
        };
        let front = draft.front.clone();

        match pipeline::clean_and_translate(&front, &config) {
            CleanOutcome::Note { note } => {
                if let Some(draft) = &mut self.draft {
                    draft.note = Some(note.clone());
                }
                notify::error("dcards", &note);
            }
            CleanOutcome::Translate {
                front,
                request,
                truncated,
            } => {
                self.next_generation = self.next_generation.wrapping_add(1);
                let generation = self.next_generation;
                if let Some(draft) = &mut self.draft {
                    draft.front = front;
                    draft.note = None;
                    draft.loading = true;
                    draft.generation = generation;
                }
                if truncated {
                    notify::info("dcards", pipeline::NOTE_TRUNCATED);
                }
                let tx = self.tx.clone();
                let repaint = self.repaint.clone();
                self.runtime.spawn(async move {
                    let result = match HttpLlmClient::new() {
                        Ok(client) => client.complete(request).await,
                        Err(err) => Err(err),
                    };
                    let event = match result {
                        Ok(back) => AppEvent::TranslationDone { generation, back },
                        Err(err) => AppEvent::TranslationFailed {
                            generation,
                            error: err.to_string(),
                        },
                    };
                    let _ = tx.send(event);
                    repaint.notify();
                });
            }
        }
    }

    /// Run the hotkey pipeline and open or refresh the draft.
    fn handle_hotkey(&mut self) {
        let selected = selection::read_selection();
        let config = self.config.clone();
        let outcome = pipeline::on_hotkey(selected.as_deref(), &config);
        let refreshing = self.draft.is_some();

        match outcome {
            DraftOutcome::Static { front, back, note } => {
                if let Some(note) = &note {
                    if note == pipeline::NOTE_TRUNCATED {
                        notify::info("dcards", note);
                    } else {
                        notify::error("dcards", note);
                    }
                } else if refreshing {
                    notify::info("dcards", "Draft updated");
                }
                self.show_draft(front, back, note, None);
            }
            DraftOutcome::Translate { front, request } => {
                if refreshing {
                    notify::info("dcards", "Draft updated");
                }
                self.show_draft(front, String::new(), None, Some(request));
            }
        }
    }

    /// Open the editor for an existing card.
    pub(crate) fn edit_card(&mut self, id: i64) {
        let cached = self.cards.iter().find(|card| card.id == id).cloned();
        if let Some(card) = cached {
            self.card_editor = Some(CardEditor::edit(&card));
            return;
        }
        // The card may be hidden by the current filter; fall back to the DB.
        match self.with_conn(|conn| cards::get(conn, id)) {
            Ok(card) => self.card_editor = Some(CardEditor::edit(&card)),
            Err(err) => self.set_error(format!("Card not found: {err}")),
        }
    }

    /// Delete a card.
    pub(crate) fn delete_card(&mut self, id: i64) {
        match self.with_conn(|conn| cards::delete(conn, id)) {
            Ok(()) => {
                if self.selected_card == Some(id) {
                    self.selected_card = None;
                }
                self.refresh_cards();
                self.set_status("Card deleted");
            }
            Err(err) => self.set_error(format!("Failed to delete card: {err}")),
        }
    }

    /// Persist the card editor contents.
    pub(crate) fn commit_card(&mut self, editor: &CardEditor) -> Result<(), String> {
        let front = editor.front.clone();
        let back = editor.back.clone();
        let group_id = editor.group_id;
        let id = editor.id;
        self.with_conn(|conn| match id {
            Some(id) => cards::update(conn, id, &front, &back, group_id),
            None => cards::create(conn, &front, &back, group_id).map(|_| ()),
        })
        .map_err(|err| err.to_string())
    }

    /// Open the group creation dialog.
    pub(crate) fn new_group(&mut self) {
        self.group_editor = Some(GroupEditor {
            mode: GroupEditorMode::Create,
            name: String::new(),
            error: None,
        });
    }

    /// Open the group rename dialog for `id`.
    pub(crate) fn rename_group(&mut self, id: i64) {
        let name = self
            .groups
            .iter()
            .find(|group| group.id == id)
            .map(|group| group.name.clone())
            .unwrap_or_default();
        self.group_editor = Some(GroupEditor {
            mode: GroupEditorMode::Rename(id),
            name,
            error: None,
        });
    }

    /// Delete a group (refused for the last or a non-empty group).
    pub(crate) fn delete_group(&mut self, id: i64) {
        match self.with_conn(|conn| groups::delete(conn, id)) {
            Ok(()) => {
                if self.selected_group == Some(id) {
                    self.selected_group = None;
                }
                self.refresh_groups();
                self.set_status("Group deleted");
            }
            Err(err) => self.set_error(format!("Failed to delete group: {err}")),
        }
    }

    /// Persist the group editor contents.
    pub(crate) fn commit_group_editor(&mut self, editor: &GroupEditor) -> Result<(), String> {
        let name = editor.name.clone();
        let result = match editor.mode {
            GroupEditorMode::Create => {
                self.with_conn(|conn| groups::create(conn, &name).map(|_| ()))
            }
            GroupEditorMode::Rename(id) => self.with_conn(|conn| groups::rename(conn, id, &name)),
        };
        match result {
            Ok(()) => {
                self.refresh_groups();
                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }

    /// Copy the settings form into the configuration and persist it.
    pub(crate) fn save_settings(&mut self, ctx: &egui::Context) {
        self.config.general.language_pair = self.settings.language_pair;
        self.config.general.default_group = self.settings.default_group.trim().to_string();
        self.config.general.cards_limit = self.settings.cards_limit.max(1);
        self.config.general.theme = self.settings.theme;
        self.config.llm.base_url = self.settings.base_url.trim().to_string();
        self.config.llm.api_key = crate::config::sanitize_api_key(&self.settings.api_key);
        self.config.llm.model = self.settings.model.trim().to_string();
        self.config.llm.timeout_secs = self.settings.timeout_secs;
        self.config.llm.max_tokens = self.settings.max_tokens;
        self.config.llm.temperature = self.settings.temperature;

        match self.config.save(&self.paths.config_file) {
            Ok(()) => {
                apply_theme(ctx, self.config.general.theme);
                db::set_slow_query_warn_ms(self.config.logging.slow_query_warn_ms);
                self.settings.status = Some("Settings saved.".to_string());
                self.set_status("Settings saved");
                self.refresh_cards();
            }
            Err(err) => {
                self.settings.status = Some(format!("Failed to save settings: {err:#}"));
            }
        }
    }

    /// Run the settings "Test connection" request against the LLM.
    ///
    /// The test word depends on the source language of the active pair. The
    /// result is delivered as [`AppEvent::TestConnectionDone`]. Nothing is
    /// written to the database.
    pub(crate) fn start_test_connection(&mut self) {
        // Test the values currently typed in the form, not the saved config.
        let mut config = self.config.clone();
        config.general.language_pair = self.settings.language_pair;
        config.llm.base_url = self.settings.base_url.trim().to_string();
        config.llm.api_key = self.settings.api_key.clone();
        config.llm.model = self.settings.model.trim().to_string();
        config.llm.timeout_secs = self.settings.timeout_secs;
        config.llm.max_tokens = self.settings.max_tokens;
        config.llm.temperature = self.settings.temperature;

        let pair: LangPair = config.general.language_pair.into();
        let word = match pair.source() {
            Lang::En => "hello",
            Lang::Ru => "привет",
        };

        if let Err(err) = validation::validate_for_pair(word, pair.source()) {
            self.test_connection = TestConnection::done(Err(err.to_string()));
            return;
        }

        let request = match llm::build_request(word, pair, &config) {
            Ok(request) => request,
            Err(err) => {
                self.test_connection = TestConnection::done(Err(err.to_string()));
                return;
            }
        };

        self.test_connection = TestConnection::loading();
        let tx = self.tx.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            let result = match HttpLlmClient::new() {
                Ok(client) => client.complete(request).await,
                Err(err) => Err(err),
            };
            let _ = tx.send(AppEvent::TestConnectionDone(
                result.map_err(|err| err.to_string()),
            ));
            repaint.notify();
        });
    }

    pub(crate) fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        tracing::warn!("{message}");
        self.status = Some(message);
    }

    fn handle_event(&mut self, ctx: &egui::Context, event: AppEvent) {
        tracing::debug!(?event, "app event");
        match event {
            AppEvent::Hotkey => {
                tracing::info!("global hotkey pressed");
                self.handle_hotkey();
            }
            AppEvent::Activate | AppEvent::TrayOpen => {
                tracing::info!("bringing the main window to the front");
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            AppEvent::TrayNewCard => {
                self.view = View::Groups;
                self.new_card();
            }
            AppEvent::TrayQuit => {
                self.quit = true;
            }
            AppEvent::TranslationDone { generation, back } => {
                if let Some(draft) = &mut self.draft {
                    if let Some(TranslationEffect::Applied { marker }) =
                        pipeline::apply_result(draft, generation, Ok(back))
                    {
                        if marker {
                            draft.note = Some("Translation unavailable".to_string());
                            notify::info("dcards", "Translation unavailable");
                        }
                    }
                }
            }
            AppEvent::TranslationFailed { generation, error } => {
                if let Some(draft) = &mut self.draft {
                    if draft.generation == generation {
                        draft.loading = false;
                        draft.note = Some(error.clone());
                        notify::error("dcards", &error);
                    }
                }
            }
            AppEvent::TestConnectionDone(result) => {
                self.test_connection = TestConnection::done(result);
            }
        }
    }

    fn ui_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("dcards");
            ui.separator();
            ui.selectable_value(&mut self.view, View::Groups, "Groups");
            ui.selectable_value(&mut self.view, View::Review, "Review");
            ui.selectable_value(&mut self.view, View::Settings, "Settings");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(status) = &self.status {
                    ui.label(status.as_str());
                }
            });
        });

        if self.wayland {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Wayland session: the global hotkey is unavailable.",
            );
        }
    }
}

impl eframe::App for DcardsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.rx.try_recv() {
            self.handle_event(ctx, event);
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| self.ui_top_bar(ui));

        match self.view {
            View::Groups => {
                egui::SidePanel::left("groups_panel")
                    .resizable(true)
                    .default_width(220.0)
                    .show(ctx, |ui| self.ui_groups_panel(ui));
                egui::CentralPanel::default().show(ctx, |ui| self.ui_cards_panel(ui));
            }
            View::Review => {
                egui::CentralPanel::default().show(ctx, |ui| self.ui_review(ui));
            }
            View::Settings => {
                egui::CentralPanel::default().show(ctx, |ui| self.ui_settings(ui));
            }
        }

        self.ui_group_editor(ctx);
        self.ui_card_editor(ctx);
        self.ui_io_dialog(ctx);
        self.ui_draft_viewport(ctx);

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

/// Current local offset from UTC, in seconds.
pub(crate) fn local_offset_seconds() -> i32 {
    chrono::Local::now().offset().local_minus_utc()
}
