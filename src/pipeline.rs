//! Hotkey processing: selection → validation → translation decision.
//!
//! This module is deliberately free of any GUI or X11 dependency so the whole
//! decision flow can be tested. [`on_hotkey`] classifies a selection and
//! [`apply_result`] folds an asynchronous translation result back into a
//! [`Draft`].

use rusqlite::Connection;

use crate::config::Config;
use crate::db::{self, cards, Card, LangPair};
use crate::llm::{self, prompt, LlmError, LlmRequest};
use crate::validation;

/// Selection contained no usable text.
pub const NOTE_NO_TEXT: &str = "No text selected";
/// Selection was longer than the limit and was truncated.
pub const NOTE_TRUNCATED: &str = "Selection truncated to 140 chars";
/// The LLM configuration is not usable.
pub const NOTE_INVALID_SETTINGS: &str = "AI settings are invalid, open Settings to configure";

/// What the UI should do after a hotkey press.
#[derive(Debug)]
pub enum DraftOutcome {
    /// Open the draft with this content; no request is made.
    Static {
        /// Front side (possibly empty).
        front: String,
        /// Back side (possibly empty).
        back: String,
        /// Optional message explaining why no request was made.
        note: Option<String>,
    },
    /// Open the draft and translate `front` with `request`.
    Translate {
        /// Front side.
        front: String,
        /// Prepared request.
        request: LlmRequest,
    },
}

/// Classify a selection according to the plan's hotkey contract.
///
/// `selection` is the already-read text (or `None` when nothing was selected).
pub fn on_hotkey(selection: Option<&str>, config: &Config) -> DraftOutcome {
    let Some(raw) = selection.map(str::trim).filter(|text| !text.is_empty()) else {
        return DraftOutcome::Static {
            front: String::new(),
            back: String::new(),
            note: Some(NOTE_NO_TEXT.to_string()),
        };
    };

    let (front, truncated) = validation::truncate_140(raw);
    if truncated {
        return DraftOutcome::Static {
            front,
            back: String::new(),
            note: Some(NOTE_TRUNCATED.to_string()),
        };
    }

    let pair: LangPair = config.general.language_pair.into();
    if let Err(err) = validation::validate_for_pair(&front, pair.source()) {
        return DraftOutcome::Static {
            front,
            back: String::new(),
            note: Some(format!("Cannot translate: {err}")),
        };
    }

    match llm::build_request(&front, pair, config) {
        Ok(request) => DraftOutcome::Translate { front, request },
        Err(_) => DraftOutcome::Static {
            front,
            back: String::new(),
            note: Some(NOTE_INVALID_SETTINGS.to_string()),
        },
    }
}

/// Result of cleaning and translating the draft's front field.
#[derive(Debug)]
pub enum CleanOutcome {
    /// Nothing was sent; `note` explains why.
    Note {
        /// Reason.
        note: String,
    },
    /// Ready to translate; `truncated` reports whether the front was cut.
    Translate {
        /// Cleaned front value.
        front: String,
        /// Prepared request.
        request: LlmRequest,
        /// Whether the front was truncated to the limit.
        truncated: bool,
    },
}

/// Apply the same cleaning rules to the draft's front field.
pub fn clean_and_translate(front: &str, config: &Config) -> CleanOutcome {
    let (front, truncated) = validation::truncate_140(front.trim());
    if front.is_empty() {
        return CleanOutcome::Note {
            note: "Enter a word to translate".to_string(),
        };
    }

    let pair: LangPair = config.general.language_pair.into();
    if let Err(err) = validation::validate_for_pair(&front, pair.source()) {
        return CleanOutcome::Note {
            note: err.to_string(),
        };
    }

    match llm::build_request(&front, pair, config) {
        Ok(request) => CleanOutcome::Translate {
            front,
            request,
            truncated,
        },
        Err(_) => CleanOutcome::Note {
            note: NOTE_INVALID_SETTINGS.to_string(),
        },
    }
}

/// Effect of an asynchronous translation on a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationEffect {
    /// `back` was updated. `marker` is set when the model reported that it
    /// could not translate the word.
    Applied {
        /// Whether the answer is the "translation unavailable" marker.
        marker: bool,
    },
    /// The request failed; `back` was left untouched.
    Failed {
        /// Error message.
        message: String,
    },
}

/// Fold a translation result into `draft`.
///
/// Returns `None` (and changes nothing) when `generation` is stale.
pub fn apply_result(
    draft: &mut Draft,
    generation: u64,
    result: Result<String, LlmError>,
) -> Option<TranslationEffect> {
    if draft.generation != generation {
        return None;
    }
    draft.loading = false;
    match result {
        Ok(back) => {
            let marker = prompt::is_marker(&back);
            draft.back = back;
            Some(TranslationEffect::Applied { marker })
        }
        Err(err) => Some(TranslationEffect::Failed {
            message: err.to_string(),
        }),
    }
}

/// State of the new-card draft window.
#[derive(Debug, Clone)]
pub struct Draft {
    /// Front side.
    pub front: String,
    /// Back side.
    pub back: String,
    /// Selected group.
    pub group_id: i64,
    /// Status message shown in the draft.
    pub note: Option<String>,
    /// Whether a translation is in flight.
    pub loading: bool,
    /// Generation of the in-flight request; stale results are ignored.
    pub generation: u64,
    /// Focus the front field on the next frame.
    pub focus_front: bool,
}

impl Draft {
    /// A blank draft in `group_id`.
    pub fn new(group_id: i64) -> Draft {
        Draft {
            front: String::new(),
            back: String::new(),
            group_id,
            note: None,
            loading: false,
            generation: 0,
            focus_front: true,
        }
    }
}

/// Insert a card from draft content into `group_id`.
///
/// Used by the draft save button; exposed separately so it can be tested
/// without any UI.
pub fn save_card(conn: &Connection, front: &str, back: &str, group_id: i64) -> db::Result<Card> {
    cards::create(conn, front, back, group_id)
}
