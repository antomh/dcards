//! Review view. The actual session is implemented in stage 5.

use eframe::egui;

use crate::app::DcardsApp;

impl DcardsApp {
    /// Placeholder for the spaced-repetition review.
    pub(crate) fn ui_review(&mut self, ui: &mut egui::Ui) {
        ui.heading("Review");
        ui.add_space(8.0);
        ui.label("The review mode arrives in stage 5.");
    }
}
