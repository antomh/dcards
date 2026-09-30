//! Card table, filter bar and the manual card dialog.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::app::DcardsApp;
use crate::ui::widgets;

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
            if ui.button("Export").clicked() {
                self.set_status("Export arrives in stage 6.");
            }
            if ui.button("Import").clicked() {
                self.set_status("Import arrives in stage 6.");
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
