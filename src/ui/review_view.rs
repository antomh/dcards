//! Review view: a simple front/back walk through a group's cards.

use eframe::egui;

use crate::app::DcardsApp;
use crate::ui::widgets;

impl DcardsApp {
    /// Review tab. Shows start controls until a session is running.
    pub(crate) fn ui_review(&mut self, ui: &mut egui::Ui) {
        let group_name = self
            .current_group()
            .map(|group| group.name.clone())
            .unwrap_or_else(|| "<no group>".to_string());

        if self.review.is_none() {
            self.ui_review_start(ui, &group_name);
        } else {
            self.ui_review_session(ui, &group_name);
        }
    }

    fn ui_review_start(&mut self, ui: &mut egui::Ui, group_name: &str) {
        ui.heading(format!("Review — {group_name}"));
        ui.add_space(8.0);
        ui.label("Optional date filter:");
        let _ = widgets::date_filter_ui(ui, &mut self.review_filter);
        ui.add_space(8.0);

        let has_group = self.selected_group.is_some();
        if ui
            .add_enabled(has_group, egui::Button::new("Start review"))
            .clicked()
        {
            self.start_review();
        }
        if !has_group {
            ui.add_space(4.0);
            ui.weak("Select a group to start a review.");
        }
    }

    fn ui_review_session(&mut self, ui: &mut egui::Ui, group_name: &str) {
        let finished = self
            .review
            .as_ref()
            .map(|session| session.is_finished())
            .unwrap_or(true);

        if finished {
            ui.heading(format!("Session complete — {group_name}"));
            ui.add_space(8.0);
            if ui.button("Restart").clicked() {
                self.start_review();
            }
            if ui.button("Back to filters").clicked() {
                self.review = None;
            }
            return;
        }

        let (position, total, front, back, revealed) = {
            let session = self.review.as_ref().expect("checked above");
            let card = session.current().expect("session is not finished");
            (
                session.position(),
                session.len(),
                card.front.clone(),
                card.back.clone(),
                session.is_revealed(),
            )
        };

        ui.horizontal(|ui| {
            ui.heading(format!("Review — {group_name}"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{position} / {total}"));
            });
        });
        ui.add_space(24.0);

        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(&front).size(28.0).strong());
            ui.add_space(24.0);

            if revealed {
                ui.separator();
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&back).size(20.0));
                ui.add_space(24.0);
                if ui.button("Next").clicked() {
                    if let Some(session) = self.review.as_mut() {
                        session.advance();
                    }
                }
            } else if ui.button("Show back").clicked() {
                if let Some(session) = self.review.as_mut() {
                    session.reveal();
                }
            }
        });
    }

    /// Start (or restart) a review session for the selected group.
    pub(crate) fn start_review(&mut self) {
        let Some(group_id) = self.selected_group else {
            self.set_status("Select a group first");
            return;
        };

        let (from, to) = self.review_filter.resolve(
            chrono::Utc::now().timestamp(),
            crate::app::local_offset_seconds(),
        );
        let filter = crate::db::CardFilter {
            group_id: Some(group_id),
            from,
            to,
        };
        let limit = self.config.general.cards_limit;
        let seed = rand::random::<u64>();

        match self
            .with_conn(|conn| crate::review::build_session(conn, group_id, &filter, limit, seed))
        {
            Ok(session) if session.is_empty() => {
                self.review = None;
                self.set_status("No cards to review");
            }
            Ok(session) => {
                let count = session.len();
                self.review = Some(session);
                self.set_status(format!("Reviewing {count} cards"));
            }
            Err(err) => {
                tracing::error!(error = %err, "failed to build review session");
                self.set_error(format!("Failed to start review: {err}"));
            }
        }
    }
}
