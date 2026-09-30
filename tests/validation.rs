//! Tests for selection validation and truncation.

use dcards::db::Lang;
use dcards::validation::{truncate_140, validate_for_pair, InvalidText, MAX_SELECTION_CHARS};

#[test]
fn english_accepts_latin_letters() {
    assert!(validate_for_pair("hello", Lang::En).is_ok());
    assert!(validate_for_pair("Hello, world!", Lang::En).is_ok());
    assert!(validate_for_pair("route 66", Lang::En).is_ok());
    assert!(validate_for_pair("café", Lang::En).is_ok());
}

#[test]
fn english_rejects_cyrillic() {
    assert!(matches!(
        validate_for_pair("привет", Lang::En),
        Err(InvalidText::ForbiddenChar('п'))
    ));
}

#[test]
fn russian_accepts_cyrillic_letters() {
    assert!(validate_for_pair("привет", Lang::Ru).is_ok());
    assert!(validate_for_pair("Привет, мир!", Lang::Ru).is_ok());
    assert!(validate_for_pair("кто-то", Lang::Ru).is_ok());
}

#[test]
fn russian_rejects_latin() {
    assert!(matches!(
        validate_for_pair("hello", Lang::Ru),
        Err(InvalidText::ForbiddenChar('h'))
    ));
}

#[test]
fn digits_only_is_rejected() {
    assert_eq!(
        validate_for_pair("12345", Lang::En),
        Err(InvalidText::NoSourceLetters)
    );
    assert_eq!(
        validate_for_pair(" 42 ", Lang::Ru),
        Err(InvalidText::NoSourceLetters)
    );
}

#[test]
fn punctuation_only_is_rejected() {
    assert_eq!(
        validate_for_pair("...!?", Lang::En),
        Err(InvalidText::NoSourceLetters)
    );
    assert_eq!(
        validate_for_pair("-'’", Lang::Ru),
        Err(InvalidText::NoSourceLetters)
    );
}

#[test]
fn basic_punctuation_is_allowed() {
    assert!(validate_for_pair("well-known, isn't it?!", Lang::En).is_ok());
    assert!(validate_for_pair("стать-то, вот!", Lang::Ru).is_ok());
}

#[test]
fn forbidden_character_is_reported() {
    assert!(matches!(
        validate_for_pair("hello@", Lang::En),
        Err(InvalidText::ForbiddenChar('@'))
    ));
    assert!(matches!(
        validate_for_pair("привет#", Lang::Ru),
        Err(InvalidText::ForbiddenChar('#'))
    ));
    // A CJK character is not allowed in either language.
    assert!(matches!(
        validate_for_pair("你好", Lang::En),
        Err(InvalidText::ForbiddenChar('你'))
    ));
}

#[test]
fn empty_text_is_rejected() {
    assert_eq!(validate_for_pair("", Lang::En), Err(InvalidText::Empty));
    assert_eq!(
        validate_for_pair("   \t", Lang::Ru),
        Err(InvalidText::Empty)
    );
}

#[test]
fn truncation_boundary_is_exact() {
    let short = "a".repeat(MAX_SELECTION_CHARS);
    let (text, truncated) = truncate_140(&short);
    assert_eq!(text, short);
    assert!(!truncated);

    let long = "a".repeat(MAX_SELECTION_CHARS + 1);
    let (text, truncated) = truncate_140(&long);
    assert!(truncated);
    assert_eq!(text.chars().count(), MAX_SELECTION_CHARS);
}

#[test]
fn truncation_is_unicode_safe() {
    let text = "ж".repeat(MAX_SELECTION_CHARS + 50);
    let (truncated_text, truncated) = truncate_140(&text);
    assert!(truncated);
    assert_eq!(truncated_text.chars().count(), MAX_SELECTION_CHARS);
    assert!(truncated_text.chars().all(|c| c == 'ж'));
}
