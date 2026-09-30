//! Settings view: general options, LLM endpoint and theme.

use eframe::egui;

use crate::app::DcardsApp;
use crate::config::{LanguagePair, Theme};
use crate::ui::language_pair_label;

impl DcardsApp {
    /// Settings form. Changes are persisted on "Save".
    pub(crate) fn ui_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.strong("General");
                egui::Grid::new("general_settings")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Language pair");
                        egui::ComboBox::from_id_salt("settings_language_pair")
                            .selected_text(language_pair_label(self.settings.language_pair))
                            .show_ui(ui, |ui| {
                                for pair in
                                    [LanguagePair::EnEn, LanguagePair::EnRu, LanguagePair::RuEn]
                                {
                                    ui.selectable_value(
                                        &mut self.settings.language_pair,
                                        pair,
                                        language_pair_label(pair),
                                    );
                                }
                            });
                        ui.end_row();

                        ui.label("Default group");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.default_group)
                                .desired_width(240.0),
                        );
                        ui.end_row();

                        ui.label("Cards limit");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.cards_limit).range(1..=100_000),
                        );
                        ui.end_row();

                        ui.label("Theme");
                        egui::ComboBox::from_id_salt("settings_theme")
                            .selected_text(theme_label(self.settings.theme))
                            .show_ui(ui, |ui| {
                                for theme in [Theme::Dark, Theme::Light] {
                                    ui.selectable_value(
                                        &mut self.settings.theme,
                                        theme,
                                        theme_label(theme),
                                    );
                                }
                            });
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.strong("LLM");
                egui::Grid::new("llm_settings")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Base URL");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.base_url)
                                .desired_width(320.0),
                        );
                        ui.end_row();

                        ui.label("API key");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.api_key)
                                .password(true)
                                .desired_width(320.0),
                        );
                        ui.end_row();

                        ui.label("Model");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.model)
                                .desired_width(240.0),
                        );
                        ui.end_row();

                        ui.label("Timeout (s)");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.timeout_secs).range(1..=3_600),
                        );
                        ui.end_row();

                        ui.label("Max tokens");
                        ui.add(
                            egui::DragValue::new(&mut self.settings.max_tokens).range(1..=4_096),
                        );
                        ui.end_row();

                        ui.label("Temperature");
                        ui.add(egui::Slider::new(&mut self.settings.temperature, 0.0..=2.0));
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let ctx = ui.ctx().clone();
                        self.save_settings(&ctx);
                    }
                    if ui.button("Test connection").clicked() {
                        self.settings.status =
                            Some("Test connection arrives in stage 3.".to_string());
                    }
                });

                if let Some(status) = &self.settings.status {
                    ui.add_space(4.0);
                    ui.label(status.as_str());
                }
            });
    }
}

fn theme_label(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "dark",
        Theme::Light => "light",
    }
}
