//! The draft window shown for new cards.
//!
//! The draft is a separate eframe viewport (its own OS window) that is always
//! on top. It is used both for hotkey-driven cards and for the manual
//! "New card" action (the latter opens it without a translation request).

use eframe::egui;

use crate::app::DcardsApp;
use crate::db::Group;
use crate::pipeline::Draft;
use crate::ui::widgets;

/// What the user asked the draft to do in this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DraftAction {
    /// Nothing.
    None,
    /// Save the card.
    Save,
    /// Discard the draft.
    Cancel,
    /// Send the front side to the LLM.
    CleanTranslate,
}

impl DcardsApp {
    /// Render the draft viewport, if a draft is open.
    pub(crate) fn ui_draft_viewport(&mut self, ctx: &egui::Context) {
        if self.draft.is_none() {
            return;
        }

        let groups = self.groups.clone();
        let viewport_id = egui::ViewportId::from_hash_of("dcards_draft");
        let builder = egui::ViewportBuilder::default()
            .with_title("dcards — new card")
            .with_always_on_top()
            .with_inner_size([480.0, 420.0]);

        let (action, close_requested) =
            ctx.show_viewport_immediate(viewport_id, builder, |ctx, class| {
                let mut action = DraftAction::None;
                let draft = self.draft.as_mut().expect("draft was checked above");

                match class {
                    egui::ViewportClass::Embedded => {
                        egui::Window::new("dcards — new card")
                            .collapsible(false)
                            .resizable(true)
                            .show(ctx, |ui| draft_body(ui, draft, &groups, &mut action));
                    }
                    _ => {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            draft_body(ui, draft, &groups, &mut action);
                        });
                    }
                }

                let close = ctx.input(|input| input.viewport().close_requested());
                (action, close)
            });

        match action {
            DraftAction::Save => self.save_draft(),
            DraftAction::Cancel => self.draft = None,
            DraftAction::CleanTranslate => self.draft_clean_translate(),
            DraftAction::None => {
                if close_requested {
                    self.draft = None;
                }
            }
        }
    }
}

/// Draw the draft form.
fn draft_body(ui: &mut egui::Ui, draft: &mut Draft, groups: &[Group], action: &mut DraftAction) {
    ui.heading("New card");
    ui.add_space(4.0);

    ui.label("Front");
    let focus_front = draft.focus_front;
    let front = ui.add(
        egui::TextEdit::multiline(&mut draft.front)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if focus_front {
        front.request_focus();
        draft.focus_front = false;
    }

    ui.add_space(4.0);
    ui.label("Back");
    ui.add(
        egui::TextEdit::multiline(&mut draft.back)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );

    if draft.loading {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Translating...");
        });
    }

    if let Some(note) = &draft.note {
        ui.add_space(4.0);
        ui.colored_label(ui.visuals().warn_fg_color, note);
    }

    ui.add_space(4.0);
    ui.label("Group");
    if groups.is_empty() {
        ui.colored_label(ui.visuals().error_fg_color, "No groups available.");
    } else {
        widgets::group_combo(ui, "draft_group", groups, &mut draft.group_id);
    }

    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        *action = DraftAction::Cancel;
    }

    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!groups.is_empty(), egui::Button::new("Save"))
            .clicked()
        {
            *action = DraftAction::Save;
        }
        if ui.button("Cancel").clicked() {
            *action = DraftAction::Cancel;
        }
        if ui.button("Clean and translate").clicked() {
            *action = DraftAction::CleanTranslate;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Group;

    #[test]
    fn draft_body_renders_without_panicking() {
        let ctx = egui::Context::default();
        let groups = vec![Group {
            id: 1,
            name: "General".to_string(),
            created_at: 0,
        }];
        let mut draft = Draft::new(1);
        draft.front = "house".to_string();
        draft.back = "дом".to_string();
        draft.loading = true;
        draft.note = Some("note".to_string());
        let mut action = DraftAction::None;

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draft_body(ui, &mut draft, &groups, &mut action);
            });
        });

        assert_eq!(action, DraftAction::None);
    }
}
