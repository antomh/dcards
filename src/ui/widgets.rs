//! Reusable widgets and the date-filter logic.
//!
//! Dates are stored as unix seconds in UTC. The filter UI, however, works in
//! the user's local time zone: "Today" means the local day containing "now",
//! and a custom `YYYY-MM-DD` bound is expanded to the corresponding local day
//! (inclusive) before being converted back to UTC.

use chrono::NaiveDate;
use eframe::egui;

use crate::db::Group;

const SECS_PER_DAY: i64 = 86_400;

/// Quick date ranges offered in the filter bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePreset {
    /// The current local day.
    Today,
    /// The current local day and the six preceding days.
    Last7,
    /// The current local day and the 29 preceding days.
    Last30,
    /// No date restriction.
    All,
    /// Bounds typed into the `from`/`to` fields.
    Custom,
}

impl DatePreset {
    /// Buttons shown in the filter bar (in order).
    pub const BUTTONS: [DatePreset; 4] = [
        DatePreset::Today,
        DatePreset::Last7,
        DatePreset::Last30,
        DatePreset::All,
    ];

    /// Short label for the button.
    pub fn label(self) -> &'static str {
        match self {
            DatePreset::Today => "Today",
            DatePreset::Last7 => "7d",
            DatePreset::Last30 => "30d",
            DatePreset::All => "All",
            DatePreset::Custom => "Custom",
        }
    }
}

/// State of the date-filter controls.
#[derive(Debug, Clone)]
pub struct DateFilterState {
    /// Selected preset.
    pub preset: DatePreset,
    /// `from` field text (`YYYY-MM-DD`).
    pub from_text: String,
    /// `to` field text (`YYYY-MM-DD`).
    pub to_text: String,
}

impl Default for DateFilterState {
    fn default() -> Self {
        DateFilterState {
            preset: DatePreset::All,
            from_text: String::new(),
            to_text: String::new(),
        }
    }
}

impl DateFilterState {
    /// Resolve the current selection to inclusive UTC bounds.
    ///
    /// See [`resolve`] for the exact semantics.
    pub fn resolve(&self, now_utc: i64, local_offset_seconds: i32) -> (Option<i64>, Option<i64>) {
        resolve(
            self.preset,
            &self.from_text,
            &self.to_text,
            now_utc,
            local_offset_seconds,
        )
    }
}

/// Resolve a preset and/or custom text bounds to inclusive UTC bounds.
///
/// `local_offset_seconds` is the current offset east of UTC (as returned by
/// `chrono::Local::now().offset().local_minus_utc()`). Presets are computed
/// from local midnight; custom bounds are interpreted as local dates, with the
/// `to` bound widened to the end of its local day. Empty or invalid custom
/// fields become `None` (no bound).
pub fn resolve(
    preset: DatePreset,
    from_text: &str,
    to_text: &str,
    now_utc: i64,
    local_offset_seconds: i32,
) -> (Option<i64>, Option<i64>) {
    let offset = i64::from(local_offset_seconds);
    match preset {
        DatePreset::All => (None, None),
        DatePreset::Today => (Some(local_day_start(now_utc, offset)), None),
        DatePreset::Last7 => (
            Some(local_day_start(now_utc, offset) - 6 * SECS_PER_DAY),
            None,
        ),
        DatePreset::Last30 => (
            Some(local_day_start(now_utc, offset) - 29 * SECS_PER_DAY),
            None,
        ),
        DatePreset::Custom => (
            parse_local_date(from_text, offset, Bound::Start),
            parse_local_date(to_text, offset, Bound::End),
        ),
    }
}

/// Start of the local day containing `now_utc`, expressed in UTC.
fn local_day_start(now_utc: i64, offset: i64) -> i64 {
    let local_seconds = now_utc + offset;
    let local_midnight = local_seconds.div_euclid(SECS_PER_DAY) * SECS_PER_DAY;
    local_midnight - offset
}

#[derive(Clone, Copy)]
enum Bound {
    /// Local midnight at the start of the day.
    Start,
    /// One second before local midnight of the following day.
    End,
}

fn parse_local_date(text: &str, offset: i64, bound: Bound) -> Option<i64> {
    let date = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()?;
    let date = match bound {
        Bound::Start => date,
        Bound::End => date.succ_opt()?,
    };
    let midnight_local = date.and_hms_opt(0, 0, 0)?.and_utc().timestamp();
    let utc = midnight_local - offset;
    Some(match bound {
        Bound::Start => utc,
        Bound::End => utc - 1,
    })
}

/// Draw the date-filter controls. Returns `true` when the selection changed.
pub fn date_filter_ui(ui: &mut egui::Ui, state: &mut DateFilterState) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Created:");
        for preset in DatePreset::BUTTONS {
            if ui
                .selectable_label(state.preset == preset, preset.label())
                .clicked()
            {
                state.preset = preset;
                state.from_text.clear();
                state.to_text.clear();
                changed = true;
            }
        }

        let from = ui.add(
            egui::TextEdit::singleline(&mut state.from_text)
                .hint_text("from YYYY-MM-DD")
                .desired_width(110.0),
        );
        let to = ui.add(
            egui::TextEdit::singleline(&mut state.to_text)
                .hint_text("to YYYY-MM-DD")
                .desired_width(110.0),
        );
        if from.changed() || to.changed() {
            state.preset = DatePreset::Custom;
            changed = true;
        }
    });
    changed
}

/// A group picker without an empty option. Returns `true` on change.
pub fn group_combo(ui: &mut egui::Ui, id_salt: &str, groups: &[Group], selected: &mut i64) -> bool {
    let mut changed = false;
    let current = groups
        .iter()
        .find(|group| group.id == *selected)
        .map(|group| group.name.clone())
        .unwrap_or_else(|| "<none>".to_string());

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for group in groups {
                if ui
                    .selectable_label(*selected == group.id, &group.name)
                    .clicked()
                    && *selected != group.id
                {
                    *selected = group.id;
                    changed = true;
                }
            }
        });
    changed
}
