//! Tests for configuration loading/saving, URL normalisation and validation.

use std::fs;
use std::os::unix::fs::PermissionsExt;

use dcards::config::{normalize_base_url, Config, LanguagePair, LlmConfigError, Theme};
use dcards::paths::{create_private_file, PRIVATE_FILE_MODE};

fn mode_of(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn load_creates_default_file_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    assert!(!path.exists());

    let config = Config::load(&path).unwrap();

    assert!(path.exists(), "config file should be created");
    assert_eq!(config.general.default_group, "General");
    assert_eq!(config.general.language_pair, LanguagePair::EnEn);
    assert_eq!(config.general.cards_limit, 500);
    assert_eq!(config.general.theme, Theme::Dark);
    assert!(!config.prompts.en_en.is_empty());
    assert!(!config.prompts.en_ru.is_empty());
    assert!(!config.prompts.ru_en.is_empty());
}

#[test]
fn save_load_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    let mut config = Config::default();
    config.general.default_group = "Custom".to_string();
    config.general.language_pair = LanguagePair::RuEn;
    config.general.cards_limit = 42;
    config.llm.model = "my-model".to_string();
    config.llm.api_key = "super-secret-key".to_string();
    config.prompts.en_en = "custom prompt".to_string();

    config.save(&path).unwrap();
    let loaded = Config::load(&path).unwrap();

    assert_eq!(loaded.general.default_group, "Custom");
    assert_eq!(loaded.general.language_pair, LanguagePair::RuEn);
    assert_eq!(loaded.general.cards_limit, 42);
    assert_eq!(loaded.llm.model, "my-model");
    assert_eq!(loaded.llm.api_key, "super-secret-key");
    assert_eq!(loaded.prompts.en_en, "custom prompt");
}

#[test]
fn config_file_has_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");

    Config::load(&path).unwrap();
    assert_eq!(mode_of(&path), PRIVATE_FILE_MODE);
}

#[test]
fn temporary_file_has_private_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("config.toml.tmp");

    let file = create_private_file(&tmp).unwrap();
    drop(file);

    assert_eq!(mode_of(&tmp), PRIVATE_FILE_MODE);
}

#[test]
fn debug_redacts_api_key() {
    let llm = dcards::config::LlmConfig {
        api_key: "super-secret-key".to_string(),
        ..Default::default()
    };

    let rendered = format!("{llm:?}");

    assert!(!rendered.contains("super-secret-key"));
    assert!(rendered.contains("***"));
}

#[test]
fn normalize_base_url_appends_endpoint() {
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1"),
        "http://localhost:11434/v1/chat/completions"
    );
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1/"),
        "http://localhost:11434/v1/chat/completions"
    );
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1/chat/completions"),
        "http://localhost:11434/v1/chat/completions"
    );
    assert_eq!(
        normalize_base_url(" https://api.example.com/v1/// "),
        "https://api.example.com/v1/chat/completions"
    );
}

#[test]
fn validate_llm_allows_localhost_without_key() {
    let config = Config::default();
    assert!(config.validate_llm().is_ok());

    let mut ip = Config::default();
    ip.llm.base_url = "http://127.0.0.1:1234/v1".to_string();
    assert!(ip.validate_llm().is_ok());

    let mut ipv6 = Config::default();
    ipv6.llm.base_url = "http://[::1]:1234/v1".to_string();
    assert!(ipv6.validate_llm().is_ok());
}

#[test]
fn validate_llm_requires_key_for_remote_host() {
    let mut config = Config::default();
    config.llm.base_url = "https://api.example.com/v1".to_string();
    assert_eq!(config.validate_llm(), Err(LlmConfigError::MissingApiKey));

    config.llm.api_key = "key".to_string();
    assert!(config.validate_llm().is_ok());
}

#[test]
fn validate_llm_rejects_invalid_base_url() {
    let mut config = Config::default();
    config.llm.base_url = String::new();
    assert_eq!(config.validate_llm(), Err(LlmConfigError::EmptyBaseUrl));

    config.llm.base_url = "not a url".to_string();
    assert!(matches!(
        config.validate_llm(),
        Err(LlmConfigError::InvalidBaseUrl(_))
    ));

    // No scheme: parses as a scheme but has no host.
    config.llm.base_url = "localhost:8080".to_string();
    assert!(matches!(
        config.validate_llm(),
        Err(LlmConfigError::InvalidBaseUrl(_))
    ));
}

#[test]
fn validate_llm_rejects_empty_model() {
    let mut config = Config::default();
    config.llm.model = "   ".to_string();
    assert_eq!(config.validate_llm(), Err(LlmConfigError::EmptyModel));
}
