//! Card table, filter bar and the manual card dialog.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::app::DcardsApp;
use crate::ui::widgets;
use crate::ui::{IoDialog, IoMode};
use crate::{export, import, notify};

impl DcardsApp {
    /// Right-hand panel: cards of the selected group.
    pub(crate) fn ui_cards_panel(&mut self, ui: &mut egui::Ui) {
        let Some(group_name) = self.current_group().map(|group| group.name.clone()) else {
            ui.centered_and_justified(|ui| ui.weak("Select or create a group."));
            return;
        };

        ui.horizontal(|ui| {
            ui.heading(&group_name);
            ui.separator();
            if ui.button("New").clicked() {
                self.new_card();
            }
            let has_card = self.selected_card.is_some();
            if ui
                .add_enabled(has_card, egui::Button::new("Edit"))
                .clicked()
            {
                if let Some(id) = self.selected_card {
                    self.edit_card(id);
                }
            }
            if ui
                .add_enabled(has_card, egui::Button::new("Delete"))
                .clicked()
            {
                if let Some(id) = self.selected_card {
                    self.delete_card(id);
                }
            }
            ui.separator();
            ui.menu_button("Export", |ui| {
                if ui.button("This group").clicked() {
                    self.io_dialog = Some(IoDialog::new(IoMode::ExportGroup));
                    ui.close_menu();
                }
                if ui.button("All cards").clicked() {
                    self.io_dialog = Some(IoDialog::new(IoMode::ExportAll));
                    ui.close_menu();
                }
            });
            if ui.button("Import TSV").clicked() {
                self.io_dialog = Some(IoDialog::new(IoMode::Import));
            }
        });

        let filter_changed = widgets::date_filter_ui(ui, &mut self.date_filter);
        ui.label(format!(
            "showing {} of {}",
            self.cards.len(),
            self.cards_total
        ));
        if filter_changed {
            self.refresh_cards();
        }

        ui.separator();

        let cards = self.cards.clone();
        let group_label = group_name.clone();
        let mut selected = self.selected_card;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder())
            .column(Column::remainder())
            .column(Column::initial(120.0))
            .column(Column::initial(150.0))
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Front");
                });
                header.col(|ui| {
                    ui.strong("Back");
                });
                header.col(|ui| {
                    ui.strong("Group");
                });
                header.col(|ui| {
                    ui.strong("Created");
                });
            })
            .body(|mut body| {
                for card in &cards {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            if ui
                                .selectable_label(selected == Some(card.id), card.front.as_str())
                                .clicked()
                            {
                                selected = Some(card.id);
                            }
                        });
                        row.col(|ui| {
                            ui.label(card.back.as_str());
                        });
                        row.col(|ui| {
                            ui.label(group_label.as_str());
                        });
                        row.col(|ui| {
                            ui.label(format_created(card.created_at));
                        });
                    });
                }
            });

        self.selected_card = selected;
    }

    /// Modal dialog for creating or editing a card.
    pub(crate) fn ui_card_editor(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.card_editor.take() else {
            return;
        };

        let groups = self.groups.clone();
        let title = if editor.id.is_some() {
            "Edit card"
        } else {
            "New card"
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Front");
                let focus_front = editor.focus_front;
                let front = ui.add(
                    egui::TextEdit::multiline(&mut editor.front)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if focus_front {
                    front.request_focus();
                    editor.focus_front = false;
                }

                ui.add_space(4.0);
                ui.label("Back");
                ui.add(
                    egui::TextEdit::multiline(&mut editor.back)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(4.0);
                ui.label("Group");
                if groups.is_empty() {
                    ui.colored_label(ui.visuals().error_fg_color, "No groups available.");
                } else {
                    widgets::group_combo(ui, "card_editor_group", &groups, &mut editor.group_id);
                }

                if let Some(error) = &editor.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!groups.is_empty(), egui::Button::new("Save"))
                        .clicked()
                    {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if save {
            match self.commit_card(&editor) {
                Ok(()) => {
                    self.selected_card = None;
                    self.refresh_cards();
                    self.set_status("Card saved");
                }
                Err(message) => {
                    editor.error = Some(message);
                    self.card_editor = Some(editor);
                }
            }
            return;
        }
        if !cancel && open {
            self.card_editor = Some(editor);
        }
    }
}

/// Format a unix timestamp in the local time zone.
fn format_created(timestamp: i64) -> String {
    use chrono::{Local, LocalResult, TimeZone};

    match Local.timestamp_opt(timestamp, 0) {
        LocalResult::Single(datetime) => datetime.format("%Y-%m-%d %H:%M").to_string(),
        _ => timestamp.to_string(),
    }
}

impl DcardsApp {
    /// Import/export dialog: a plain path field and a confirm button.
    pub(crate) fn ui_io_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.io_dialog.take() else {
            return;
        };

        let mut open = true;
        let mut run = false;
        let mut cancel = false;

        egui::Window::new(dialog.title())
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("File path");
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.path)
                        .hint_text("/home/user/cards.tsv")
                        .desired_width(f32::INFINITY),
                );

                if let Some(message) = &dialog.message {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(dialog.action_label()).clicked() {
                        run = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if run {
            self.io_dialog = Some(dialog);
            self.run_io();
            return;
        }
        if !cancel && open {
            self.io_dialog = Some(dialog);
        }
    }

    /// Run the import/export operation described by the current dialog.
    fn run_io(&mut self) {
        let Some(mut dialog) = self.io_dialog.take() else {
            return;
        };

        let path_text = dialog.path.trim().to_string();
        if path_text.is_empty() {
            dialog.message = Some("Enter a file path.".to_string());
            self.io_dialog = Some(dialog);
            return;
        }
        let path = std::path::PathBuf::from(&path_text);

        let result: Result<String, String> = match dialog.mode {
            IoMode::ExportGroup => match self.selected_group {
                None => Err("Select a group first.".to_string()),
                Some(group_id) => self
                    .with_conn_raw(|conn| export::export_group(conn, group_id, &path))
                    .map(|count| format!("Exported {count} cards to {path_text}"))
                    .map_err(|err| err.to_string()),
            },
            IoMode::ExportAll => self
                .with_conn_raw(|conn| export::export_all(conn, &path))
                .map(|count| format!("Exported {count} cards to {path_text}"))
                .map_err(|err| err.to_string()),
            IoMode::Import => {
                let default_group = self.config.general.default_group.clone();
                match self.with_conn_raw(|conn| import::import_tsv(conn, &path, &default_group)) {
                    Ok(summary) => {
                        self.refresh_groups();
                        Ok(format!(
                            "Imported {} cards ({} new groups)",
                            summary.cards, summary.groups_created
                        ))
                    }
                    Err(err) => Err(err.to_string()),
                }
            }
        };

        match result {
            Ok(message) => {
                self.set_status(message);
                self.io_dialog = None;
            }
            Err(message) => {
                tracing::error!(error = %message, "import/export failed");
                notify::error("dcards", &message);
                dialog.message = Some(format!("Error: {message}"));
                self.io_dialog = Some(dialog);
            }
        }
    }
}
