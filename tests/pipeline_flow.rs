//! End-to-end tests for the hotkey pipeline (no GUI, no X11).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use dcards::config::Config;
use dcards::db::{self, cards, groups};
use dcards::llm::{LlmClient, LlmError, LlmRequest};
use dcards::pipeline::{
    self, CleanOutcome, Draft, DraftOutcome, TranslationEffect, NOTE_INVALID_SETTINGS,
    NOTE_NO_TEXT, NOTE_TRUNCATED,
};
use rusqlite::Connection;

/// A client that returns a fixed result and counts calls.
struct FakeLlmClient {
    result: Result<String, LlmError>,
    calls: Arc<AtomicUsize>,
}

impl FakeLlmClient {
    fn new(result: Result<String, LlmError>) -> FakeLlmClient {
        FakeLlmClient {
            result,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl LlmClient for FakeLlmClient {
    async fn complete(&self, _request: LlmRequest) -> Result<String, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
}

fn setup_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let conn = db::open(&dir.path().join("dcards.db")).unwrap();
    db::migrate(&conn).unwrap();
    db::bootstrap_default_group(&conn, "General").unwrap();
    (dir, conn)
}

#[tokio::test]
async fn happy_path_translates_and_saves() {
    let (_dir, conn) = setup_db();
    let group = groups::find_by_name(&conn, "General").unwrap().unwrap();
    let config = Config::default();

    let DraftOutcome::Translate { front, request } = pipeline::on_hotkey(Some("house"), &config)
    else {
        panic!("expected a translation");
    };
    assert_eq!(front, "house");

    let client = FakeLlmClient::new(Ok("дом".to_string()));
    let back = client.complete(request).await.unwrap();
    assert_eq!(back, "дом");
    assert_eq!(client.calls.load(Ordering::SeqCst), 1);

    let mut draft = Draft::new(group.id);
    draft.front = front;
    draft.generation = 1;
    draft.loading = true;

    let effect = pipeline::apply_result(&mut draft, 1, Ok(back)).unwrap();
    assert_eq!(effect, TranslationEffect::Applied { marker: false });
    assert_eq!(draft.back, "дом");
    assert!(!draft.loading);

    let card = pipeline::save_card(&conn, &draft.front, &draft.back, draft.group_id).unwrap();
    assert_eq!(card.group_id, group.id);
    assert_eq!(cards::get(&conn, card.id).unwrap().back, "дом");
}

#[test]
fn long_selection_is_truncated_without_a_request() {
    let config = Config::default();
    let long = "a".repeat(200);

    match pipeline::on_hotkey(Some(&long), &config) {
        DraftOutcome::Static { front, back, note } => {
            assert_eq!(front.chars().count(), 140);
            assert!(back.is_empty());
            assert_eq!(note.as_deref(), Some(NOTE_TRUNCATED));
        }
        DraftOutcome::Translate { .. } => {
            panic!("no request may be made for a truncated selection")
        }
    }
}

#[test]
fn invalid_text_is_rejected() {
    let config = Config::default(); // en-en
    match pipeline::on_hotkey(Some("привет"), &config) {
        DraftOutcome::Static { front, back, note } => {
            assert_eq!(front, "привет");
            assert!(back.is_empty());
            assert!(
                note.unwrap().starts_with("Cannot translate"),
                "note should carry the reason"
            );
        }
        DraftOutcome::Translate { .. } => panic!("invalid text must not be sent"),
    }
}

#[test]
fn empty_selection_opens_an_empty_draft() {
    let config = Config::default();
    for selection in [None, Some("   "), Some("")] {
        match pipeline::on_hotkey(selection, &config) {
            DraftOutcome::Static { front, back, note } => {
                assert!(front.is_empty());
                assert!(back.is_empty());
                assert_eq!(note.as_deref(), Some(NOTE_NO_TEXT));
            }
            DraftOutcome::Translate { .. } => panic!("no text must not be sent"),
        }
    }
}

#[test]
fn invalid_settings_produce_a_note() {
    let mut config = Config::default();
    config.llm.base_url = "https://api.example.com/v1".to_string();
    config.llm.api_key = String::new();

    match pipeline::on_hotkey(Some("house"), &config) {
        DraftOutcome::Static { note, .. } => {
            assert_eq!(note.as_deref(), Some(NOTE_INVALID_SETTINGS));
        }
        DraftOutcome::Translate { .. } => panic!("misconfigured client must not be used"),
    }
}

#[tokio::test]
async fn llm_error_leaves_back_empty() {
    let config = Config::default();
    let DraftOutcome::Translate { front, request } = pipeline::on_hotkey(Some("house"), &config)
    else {
        panic!("expected a translation");
    };

    let client = FakeLlmClient::new(Err(LlmError::Network("boom".to_string())));
    let err = client.complete(request).await.unwrap_err();

    let mut draft = Draft::new(1);
    draft.front = front;
    draft.generation = 5;
    draft.loading = true;

    let effect = pipeline::apply_result(&mut draft, 5, Err(err)).unwrap();
    assert!(matches!(effect, TranslationEffect::Failed { .. }));
    assert!(draft.back.is_empty());
    assert!(!draft.loading);
}

#[tokio::test]
async fn marker_is_kept_and_flagged() {
    let config = Config::default();
    let DraftOutcome::Translate { request, .. } = pipeline::on_hotkey(Some("house"), &config)
    else {
        panic!("expected a translation");
    };

    let client = FakeLlmClient::new(Ok("Translation unavailable.".to_string()));
    let back = client.complete(request).await.unwrap();

    let mut draft = Draft::new(1);
    draft.generation = 7;

    let effect = pipeline::apply_result(&mut draft, 7, Ok(back)).unwrap();
    assert_eq!(effect, TranslationEffect::Applied { marker: true });
    assert_eq!(draft.back, "Translation unavailable.");
}

#[test]
fn stale_generation_is_ignored() {
    let mut draft = Draft::new(1);
    draft.generation = 2;
    draft.back = "old".to_string();

    assert!(pipeline::apply_result(&mut draft, 1, Ok("new".to_string())).is_none());
    assert_eq!(draft.back, "old");

    assert!(pipeline::apply_result(&mut draft, 2, Ok("new".to_string())).is_some());
    assert_eq!(draft.back, "new");
}

#[test]
fn save_writes_a_card_with_a_group() {
    let (_dir, conn) = setup_db();
    let group = groups::find_by_name(&conn, "General").unwrap().unwrap();

    let card = pipeline::save_card(&conn, "house", "дом", group.id).unwrap();
    assert_eq!(card.group_id, group.id);
    assert_eq!(cards::get(&conn, card.id).unwrap().group_id, group.id);

    // An unknown group is rejected (group_id is NOT NULL and FK-enforced).
    assert!(pipeline::save_card(&conn, "x", "", 9999).is_err());
}

#[test]
fn clean_and_translate_applies_the_same_rules() {
    let config = Config::default();

    match pipeline::clean_and_translate("  house  ", &config) {
        CleanOutcome::Translate {
            front, truncated, ..
        } => {
            assert_eq!(front, "house");
            assert!(!truncated);
        }
        CleanOutcome::Note { note } => panic!("unexpected note: {note}"),
    }

    assert!(matches!(
        pipeline::clean_and_translate("привет", &config),
        CleanOutcome::Note { .. }
    ));
    assert!(matches!(
        pipeline::clean_and_translate("   ", &config),
        CleanOutcome::Note { .. }
    ));

    let long = "a".repeat(200);
    match pipeline::clean_and_translate(&long, &config) {
        CleanOutcome::Translate {
            front, truncated, ..
        } => {
            assert!(truncated);
            assert_eq!(front.chars().count(), 140);
        }
        CleanOutcome::Note { note } => panic!("unexpected note: {note}"),
    }
}
