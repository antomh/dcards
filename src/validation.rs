//! Validation and truncation of selected text.
//!
//! The selected word is checked against the source language of the active pair
//! before it is sent to the LLM: it must contain at least one letter of the
//! expected script, and may otherwise contain digits, whitespace and a small
//! set of punctuation.
//!
//! Only the first [`MAX_SELECTION_CHARS`] characters are ever used.

use crate::db::Lang;

/// Maximum number of characters taken from a selection.
pub const MAX_SELECTION_CHARS: usize = 140;

/// Reasons a piece of text is not acceptable for a language pair.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidText {
    /// The text is empty or whitespace only.
    #[error("no text to translate")]
    Empty,
    /// The text contains no letters of the source language.
    #[error("the text does not contain any letters of the source language")]
    NoSourceLetters,
    /// The text contains a character that is not allowed.
    #[error("the text contains an unsupported character: {0:?}")]
    ForbiddenChar(char),
}

/// Truncate `text` to [`MAX_SELECTION_CHARS`] characters.
///
/// Returns the (possibly truncated) string and whether truncation happened.
/// Truncation is not word-aware and never splits a UTF-8 character.
pub fn truncate_140(text: &str) -> (String, bool) {
    if text.chars().count() > MAX_SELECTION_CHARS {
        (text.chars().take(MAX_SELECTION_CHARS).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

/// Validate `text` as a source word for `source`.
///
/// Allowed characters are letters of the source script, ASCII digits,
/// whitespace and the punctuation `- ' ’ . , ! ?`. At least one source-script
/// letter is required, which also rejects digit-only and punctuation-only
/// input.
pub fn validate_for_pair(text: &str, source: Lang) -> Result<(), InvalidText> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(InvalidText::Empty);
    }

    let mut has_letter = false;
    for ch in trimmed.chars() {
        if is_source_letter(ch, source) {
            has_letter = true;
        } else if ch.is_ascii_digit() || ch.is_whitespace() || is_allowed_punctuation(ch) {
            // Allowed, but does not count as a letter.
        } else {
            return Err(InvalidText::ForbiddenChar(ch));
        }
    }

    if !has_letter {
        return Err(InvalidText::NoSourceLetters);
    }
    Ok(())
}

fn is_allowed_punctuation(ch: char) -> bool {
    matches!(ch, '-' | '\'' | '\u{2019}' | '.' | ',' | '!' | '?')
}

fn is_source_letter(ch: char, source: Lang) -> bool {
    match source {
        // ASCII letters plus Latin-1 Supplement / Latin Extended.
        Lang::En => ch.is_ascii_alphabetic() || ('\u{00C0}'..='\u{024F}').contains(&ch),
        // Cyrillic and Cyrillic Supplement.
        Lang::Ru => {
            ('\u{0400}'..='\u{04FF}').contains(&ch) || ('\u{0500}'..='\u{052F}').contains(&ch)
        }
    }
}
