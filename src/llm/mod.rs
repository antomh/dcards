//! OpenAI-compatible chat-completions client.
//!
//! One request produces one short answer (a translation or a definition). The
//! request is built by [`build_request`] and executed by any [`LlmClient`]
//! implementation; [`HttpLlmClient`] is the real one, and tests substitute a
//! mock.

pub mod error;
pub mod prompt;

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{normalize_base_url, Config, LlmConfigError};
use crate::db::LangPair;

pub use error::LlmError;

/// A fully prepared chat-completion request.
#[derive(Clone)]
pub struct LlmRequest {
    /// Absolute endpoint URL.
    pub url: String,
    /// System prompt.
    pub system_prompt: String,
    /// User message (the word itself).
    pub user_message: String,
    /// Model name.
    pub model: String,
    /// Maximum generated tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Request timeout.
    pub timeout: Duration,
    /// Bearer token; empty means "send no Authorization header".
    pub api_key: String,
}

// Manual `Debug` so the API key cannot leak through `{:?}`.
impl std::fmt::Debug for LlmRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let api_key = if self.api_key.is_empty() { "" } else { "***" };
        f.debug_struct("LlmRequest")
            .field("url", &self.url)
            .field("system_prompt", &self.system_prompt)
            .field("user_message", &self.user_message)
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("timeout", &self.timeout)
            .field("api_key", &api_key)
            .finish()
    }
}

/// Anything that can perform a chat completion.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send `request` and return the cleaned assistant text.
    async fn complete(&self, request: LlmRequest) -> Result<String, LlmError>;
}

/// Real client backed by `reqwest`.
pub struct HttpLlmClient {
    client: reqwest::Client,
}

impl HttpLlmClient {
    /// Build a client with a default connection pool.
    pub fn new() -> Result<Self, LlmError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|err| LlmError::Network(err.to_string()))?;
        Ok(Self { client })
    }

    /// Wrap an existing `reqwest` client (used by tests).
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpLlmClient {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for HttpLlmClient {
    async fn complete(&self, request: LlmRequest) -> Result<String, LlmError> {
        let body = ChatRequest {
            model: &request.model,
            messages: [
                ChatMessage {
                    role: "system",
                    content: &request.system_prompt,
                },
                ChatMessage {
                    role: "user",
                    content: &request.user_message,
                },
            ],
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: false,
        };

        let mut builder = self
            .client
            .post(&request.url)
            .json(&body)
            .timeout(request.timeout);
        if !request.api_key.is_empty() {
            builder = builder.bearer_auth(&request.api_key);
        }

        let response = builder.send().await.map_err(map_reqwest_error)?;
        let status = response.status();
        let text = response.text().await.map_err(map_reqwest_error)?;

        if !status.is_success() {
            return Err(LlmError::Http {
                status: status.as_u16(),
                body_snippet: snippet(&text),
            });
        }

        let parsed: ChatResponse =
            serde_json::from_str(&text).map_err(|err| LlmError::Parse(err.to_string()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .unwrap_or_default();
        let cleaned = clean_content(&content);
        if cleaned.is_empty() {
            return Err(LlmError::EmptyResponse);
        }
        Ok(cleaned)
    }
}

/// Build a request for `word` using `pair` and `config`.
///
/// Fails when the LLM configuration is unusable (missing key for a remote
/// endpoint, invalid URL, empty model).
pub fn build_request(word: &str, pair: LangPair, config: &Config) -> Result<LlmRequest, LlmError> {
    config.validate_llm().map_err(map_config_error)?;

    let (source_lang, target_lang) = pair.lang_names();
    let template = prompt::template_for(&config.prompts, pair);
    let system_prompt = prompt::substitute(template, source_lang, target_lang, word);

    Ok(LlmRequest {
        url: normalize_base_url(&config.llm.base_url),
        system_prompt,
        user_message: word.to_string(),
        model: config.llm.model.clone(),
        max_tokens: config.llm.max_tokens,
        temperature: config.llm.temperature,
        timeout: Duration::from_secs(config.llm.timeout_secs),
        api_key: config.llm.api_key.clone(),
    })
}

fn map_config_error(error: LlmConfigError) -> LlmError {
    match error {
        LlmConfigError::MissingApiKey => LlmError::MissingApiKey,
        other => LlmError::InvalidConfig(other.to_string()),
    }
}

fn map_reqwest_error(error: reqwest::Error) -> LlmError {
    if error.is_timeout() {
        LlmError::Timeout
    } else {
        LlmError::Network(error.to_string())
    }
}

/// Keep at most 256 characters of a response body.
fn snippet(body: &str) -> String {
    body.chars().take(256).collect()
}

/// Trim, then drop one pair of matching surrounding quotes, then trim again.
fn clean_content(raw: &str) -> String {
    strip_surrounding_quotes(raw.trim()).trim().to_string()
}

fn strip_surrounding_quotes(text: &str) -> &str {
    let mut chars = text.chars();
    let (Some(first), Some(last)) = (chars.next(), chars.next_back()) else {
        return text;
    };
    if text.chars().count() < 2 {
        return text;
    }
    let pairs = [
        ('"', '"'),
        ('\'', '\''),
        ('\u{201C}', '\u{201D}'), // “ ”
        ('\u{2018}', '\u{2019}'), // ‘ ’
        ('\u{00AB}', '\u{00BB}'), // « »
    ];
    if pairs
        .iter()
        .any(|(open, close)| *open == first && *close == last)
    {
        &text[first.len_utf8()..text.len() - last.len_utf8()]
    } else {
        text
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
    max_tokens: u32,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
}
