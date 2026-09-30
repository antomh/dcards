//! Group list panel and the create/rename group dialog.

use eframe::egui;

use crate::app::DcardsApp;
use crate::ui::GroupEditorMode;

impl DcardsApp {
    /// Left-hand panel: group list and its toolbar.
    pub(crate) fn ui_groups_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Groups");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.new_group();
            }
            let has_selection = self.selected_group.is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("Rename"))
                .clicked()
            {
                if let Some(id) = self.selected_group {
                    self.rename_group(id);
                }
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                if let Some(id) = self.selected_group {
                    self.delete_group(id);
                }
            }
        });
        ui.separator();

        let groups = self.groups.clone();
        let selected_id = self.selected_group;
        let mut clicked = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if groups.is_empty() {
                    ui.weak("No groups yet.");
                }
                for group in &groups {
                    if ui
                        .selectable_label(selected_id == Some(group.id), group.name.as_str())
                        .clicked()
                    {
                        clicked = Some(group.id);
                    }
                }
            });

        if let Some(id) = clicked {
            if self.selected_group != Some(id) {
                self.selected_group = Some(id);
                self.selected_card = None;
                self.refresh_cards();
            }
        }
    }

    /// Modal dialog for creating or renaming a group.
    pub(crate) fn ui_group_editor(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.group_editor.take() else {
            return;
        };

        let title = match editor.mode {
            GroupEditorMode::Create => "New group",
            GroupEditorMode::Rename(_) => "Rename group",
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut editor.name).desired_width(f32::INFINITY))
                    .request_focus();

                if let Some(error) = &editor.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if save {
            match self.commit_group_editor(&editor) {
                Ok(()) => self.set_status("Group saved"),
                Err(message) => {
                    editor.error = Some(message);
                    self.group_editor = Some(editor);
                }
            }
            return;
        }
        if !cancel && open {
            self.group_editor = Some(editor);
        }
    }
}
