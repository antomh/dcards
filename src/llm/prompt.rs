//! System-prompt templates and helpers.

use crate::config::PromptsConfig;
use crate::db::LangPair;

/// Default system prompt for `en-en`.
pub const DEFAULT_EN_EN: &str = "You are a dictionary. Reply with a concise English definition of \
the word. Reply with a single line only, no quotes. If you cannot define it, reply exactly: \
Translation unavailable";

/// Default system prompt for `en-ru`.
pub const DEFAULT_EN_RU: &str = "You are a dictionary. Reply with only the Russian translation of \
the word. Reply with a single line only, no quotes. If you cannot translate it, reply exactly: \
Translation unavailable";

/// Default system prompt for `ru-en`.
pub const DEFAULT_RU_EN: &str = "You are a dictionary. Reply with only the English translation of \
the word. Reply with a single line only, no quotes. If you cannot translate it, reply exactly: \
Translation unavailable";

/// Literal the model is asked to return when it cannot translate or define the
/// word. Recognised case-insensitively by [`is_marker`].
pub const DEFAULT_MARKER: &str = "Translation unavailable";

/// The configured template for `pair`.
pub fn template_for(prompts: &PromptsConfig, pair: LangPair) -> &str {
    match pair {
        LangPair::EnEn => &prompts.en_en,
        LangPair::EnRu => &prompts.en_ru,
        LangPair::RuEn => &prompts.ru_en,
    }
}

/// Substitute the `{source_lang}`, `{target_lang}` and `{word}` placeholders.
///
/// The default templates do not use placeholders, so this is a no-op for them.
pub fn substitute(template: &str, source_lang: &str, target_lang: &str, word: &str) -> String {
    template
        .replace("{source_lang}", source_lang)
        .replace("{target_lang}", target_lang)
        .replace("{word}", word)
}

/// Whether `text` is the "translation unavailable" marker.
///
/// Matches after trimming, lower-casing and dropping a trailing period.
pub fn is_marker(text: &str) -> bool {
    text.trim().to_lowercase().trim_end_matches('.').trim() == DEFAULT_MARKER.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_replaces_all_placeholders() {
        let result = substitute(
            "Translate {word} from {source_lang} to {target_lang}.",
            "English",
            "Russian",
            "heart",
        );
        assert_eq!(result, "Translate heart from English to Russian.");
    }

    #[test]
    fn substitute_replaces_repeated_placeholders() {
        assert_eq!(substitute("{word}-{word}", "x", "y", "foo"), "foo-foo");
    }

    #[test]
    fn substitute_without_placeholders_is_unchanged() {
        assert_eq!(substitute("plain text", "a", "b", "c"), "plain text");
    }

    #[test]
    fn marker_detection() {
        assert!(is_marker("Translation unavailable"));
        assert!(is_marker("  translation unavailable.  "));
        assert!(is_marker("TRANSLATION UNAVAILABLE"));
        assert!(!is_marker("Translation"));
        assert!(!is_marker("сердце"));
        assert!(!is_marker(""));
    }
}
