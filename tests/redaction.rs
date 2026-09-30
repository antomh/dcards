//! The API key must never appear in any `Debug` output or error message.

use std::time::Duration;

use dcards::config::Config;
use dcards::llm::LlmRequest;

const SECRET: &str = "sk-super-secret-12345";

#[test]
fn config_debug_redacts_the_key() {
    let mut config = Config::default();
    config.llm.api_key = SECRET.to_string();

    let rendered = format!("{config:?}");
    assert!(!rendered.contains(SECRET), "Config Debug leaked the key");
    assert!(rendered.contains("***"));
}

#[test]
fn llm_config_debug_redacts_the_key() {
    let mut config = Config::default();
    config.llm.api_key = SECRET.to_string();

    let rendered = format!("{:?}", config.llm);
    assert!(!rendered.contains(SECRET));
    assert!(rendered.contains("***"));
}

#[test]
fn llm_request_debug_redacts_the_key() {
    let request = LlmRequest {
        url: "https://example.com/api/v1/chat/completions".to_string(),
        system_prompt: "You are a dictionary.".to_string(),
        user_message: "hello".to_string(),
        model: "test-model".to_string(),
        max_tokens: 80,
        temperature: 0.2,
        timeout: Duration::from_secs(5),
        api_key: SECRET.to_string(),
    };

    let rendered = format!("{request:?}");
    assert!(
        !rendered.contains(SECRET),
        "LlmRequest Debug leaked the key"
    );
    assert!(rendered.contains("***"));
}

#[test]
fn settings_form_debug_redacts_the_key() {
    let mut config = Config::default();
    config.llm.api_key = SECRET.to_string();

    let form = dcards::ui::SettingsForm::from_config(&config);
    let rendered = format!("{form:?}");
    assert!(
        !rendered.contains(SECRET),
        "SettingsForm Debug leaked the key"
    );
    assert!(rendered.contains("***"));
}
