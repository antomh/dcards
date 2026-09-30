//! egui views and shared UI state.

pub mod cards_view;
pub mod draft;
pub mod groups_view;
pub mod review_view;
pub mod settings_view;
pub mod widgets;

use crate::config::{Config, LanguagePair, Theme};
use crate::db::Card;

/// Top-level view selected in the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Main view: group list and the cards of the selected group.
    Groups,
    /// Spaced repetition review (implemented in stage 5).
    Review,
    /// Application and LLM settings.
    Settings,
}

/// Editor state for the manual card dialog.
#[derive(Debug, Clone)]
pub struct CardEditor {
    /// `None` when creating, `Some(id)` when editing an existing card.
    pub id: Option<i64>,
    /// Front side.
    pub front: String,
    /// Back side.
    pub back: String,
    /// Selected group.
    pub group_id: i64,
    /// Last error message, if any.
    pub error: Option<String>,
    /// Focus the front field on the next frame.
    pub focus_front: bool,
}

impl CardEditor {
    /// A blank editor for a new card in `group_id`.
    pub fn new(group_id: i64) -> CardEditor {
        CardEditor {
            id: None,
            front: String::new(),
            back: String::new(),
            group_id,
            error: None,
            focus_front: true,
        }
    }

    /// An editor pre-filled from an existing card.
    pub fn edit(card: &Card) -> CardEditor {
        CardEditor {
            id: Some(card.id),
            front: card.front.clone(),
            back: card.back.clone(),
            group_id: card.group_id,
            error: None,
            focus_front: true,
        }
    }
}

/// Whether the group editor creates a new group or renames an existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupEditorMode {
    /// Create a new group.
    Create,
    /// Rename the group with this id.
    Rename(i64),
}

/// Editor state for the create/rename group dialog.
#[derive(Debug, Clone)]
pub struct GroupEditor {
    /// Operation performed on save.
    pub mode: GroupEditorMode,
    /// Group name.
    pub name: String,
    /// Last error message, if any.
    pub error: Option<String>,
}

/// Editable copy of the settings shown in the settings view.
#[derive(Debug, Clone)]
pub struct SettingsForm {
    /// Active language pair.
    pub language_pair: LanguagePair,
    /// Default group name.
    pub default_group: String,
    /// Maximum cards shown per group.
    pub cards_limit: usize,
    /// Colour theme.
    pub theme: Theme,
    /// LLM base URL.
    pub base_url: String,
    /// LLM API key.
    pub api_key: String,
    /// LLM model.
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum generated tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Feedback shown under the form.
    pub status: Option<String>,
}

impl SettingsForm {
    /// Snapshot the editable fields from the current configuration.
    pub fn from_config(config: &Config) -> SettingsForm {
        SettingsForm {
            language_pair: config.general.language_pair,
            default_group: config.general.default_group.clone(),
            cards_limit: config.general.cards_limit,
            theme: config.general.theme,
            base_url: config.llm.base_url.clone(),
            api_key: config.llm.api_key.clone(),
            model: config.llm.model.clone(),
            timeout_secs: config.llm.timeout_secs,
            max_tokens: config.llm.max_tokens,
            temperature: config.llm.temperature,
            status: None,
        }
    }
}

/// Which import/export operation the dialog will run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoMode {
    /// Export the cards of the selected group.
    ExportGroup,
    /// Export every card.
    ExportAll,
    /// Import a TSV file.
    Import,
}

/// State of the import/export dialog.
#[derive(Debug, Clone)]
pub struct IoDialog {
    /// Operation to run.
    pub mode: IoMode,
    /// File path entered by the user.
    pub path: String,
    /// Last result or error message.
    pub message: Option<String>,
}

impl IoDialog {
    /// A dialog for `mode` with an empty path.
    pub fn new(mode: IoMode) -> IoDialog {
        IoDialog {
            mode,
            path: String::new(),
            message: None,
        }
    }

    /// Window title for the mode.
    pub fn title(&self) -> &'static str {
        match self.mode {
            IoMode::ExportGroup => "Export this group",
            IoMode::ExportAll => "Export all cards",
            IoMode::Import => "Import TSV",
        }
    }

    /// Label of the confirm button.
    pub fn action_label(&self) -> &'static str {
        match self.mode {
            IoMode::ExportGroup | IoMode::ExportAll => "Export",
            IoMode::Import => "Import",
        }
    }
}

/// State of the settings "Test connection" request.
#[derive(Debug, Clone, Default)]
pub struct TestConnection {
    /// Whether a request is currently in flight.
    pub loading: bool,
    /// Completed result: `Ok(answer)` or `Err(message)`.
    pub result: Option<Result<String, String>>,
}

impl TestConnection {
    /// A request is currently running.
    pub fn loading() -> TestConnection {
        TestConnection {
            loading: true,
            result: None,
        }
    }

    /// A request finished with `result`.
    pub fn done(result: Result<String, String>) -> TestConnection {
        TestConnection {
            loading: false,
            result: Some(result),
        }
    }
}

/// Human-readable label for a language pair.
pub fn language_pair_label(pair: LanguagePair) -> &'static str {
    match pair {
        LanguagePair::EnEn => "en-en",
        LanguagePair::EnRu => "en-ru",
        LanguagePair::RuEn => "ru-en",
    }
}
