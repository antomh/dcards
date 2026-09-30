//! Application configuration (`~/.config/dcards/config.toml`).
//!
//! The file is created on first run with `0600` permissions. Saving is atomic
//! (temporary file with `0600`, then rename) and never logs the API key.

use std::fmt;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::paths::{create_private_file, set_private};

/// Default group created on first run and pre-selected in the card draft.
pub const DEFAULT_GROUP_NAME: &str = "General";

/// BCP-ish identifier of one of the supported language pairs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanguagePair {
    /// English source, English definition.
    #[default]
    #[serde(rename = "en-en")]
    EnEn,
    /// English source, Russian translation.
    #[serde(rename = "en-ru")]
    EnRu,
    /// Russian source, English translation.
    #[serde(rename = "ru-en")]
    RuEn,
}

impl LanguagePair {
    /// The language the source word is expected to be written in.
    pub fn source_is_cyrillic(self) -> bool {
        matches!(self, LanguagePair::RuEn)
    }
}

/// Colour theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    /// Dark visuals.
    #[default]
    #[serde(rename = "dark")]
    Dark,
    /// Light visuals.
    #[serde(rename = "light")]
    Light,
}

/// General application behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Active language pair.
    pub language_pair: LanguagePair,
    /// Default group name (also used for the first-run bootstrap).
    pub default_group: String,
    /// Maximum number of cards shown for a single group.
    pub cards_limit: usize,
    /// Colour theme.
    pub theme: Theme,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        GeneralConfig {
            language_pair: LanguagePair::default(),
            default_group: DEFAULT_GROUP_NAME.to_string(),
            cards_limit: 500,
            theme: Theme::default(),
        }
    }
}

/// Storage locations. All fields are optional overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Override for the SQLite database path.
    pub db_path: Option<PathBuf>,
}

/// OpenAI-compatible LLM endpoint settings.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// Base URL of the API, e.g. `http://localhost:11434/v1`.
    pub base_url: String,
    /// Bearer token. Required unless `base_url` points at localhost.
    pub api_key: String,
    /// Model name.
    pub model: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum number of generated tokens.
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        LlmConfig {
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: String::new(),
            model: "llama3.1".to_string(),
            timeout_secs: 15,
            max_tokens: 80,
            temperature: 0.2,
        }
    }
}

// Manual `Debug` so the API key can never leak through `{:?}` (logs, panics,
// error contexts).
impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let api_key = if self.api_key.is_empty() { "" } else { "***" };
        f.debug_struct("LlmConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &api_key)
            .field("model", &self.model)
            .field("timeout_secs", &self.timeout_secs)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .finish()
    }
}

/// System-prompt templates per language pair (TOML keys `en_en`, `en_ru`,
/// `ru_en`). Placeholders `{source_lang}`, `{target_lang}` and `{word}` are
/// substituted at request time; the defaults below do not use them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptsConfig {
    /// System prompt for `en-en`.
    pub en_en: String,
    /// System prompt for `en-ru`.
    pub en_ru: String,
    /// System prompt for `ru-en`.
    pub ru_en: String,
}

impl Default for PromptsConfig {
    fn default() -> Self {
        PromptsConfig {
            en_en: crate::llm::prompt::DEFAULT_EN_EN.to_string(),
            en_ru: crate::llm::prompt::DEFAULT_EN_RU.to_string(),
            ru_en: crate::llm::prompt::DEFAULT_RU_EN.to_string(),
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// `tracing` filter directive, e.g. `info` or `dcards=debug`.
    pub level: String,
    /// Number of daily log files to retain.
    pub max_files: usize,
    /// Log a warning when a database operation exceeds this duration.
    pub slow_query_warn_ms: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            level: "info".to_string(),
            max_files: 7,
            slow_query_warn_ms: 50,
        }
    }
}

/// The complete configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// General behaviour.
    pub general: GeneralConfig,
    /// Storage overrides.
    pub storage: StorageConfig,
    /// LLM endpoint.
    pub llm: LlmConfig,
    /// Prompt templates.
    pub prompts: PromptsConfig,
    /// Logging.
    pub logging: LoggingConfig,
}

/// Errors produced while validating the LLM section.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlmConfigError {
    /// `base_url` is empty.
    #[error("base_url is empty")]
    EmptyBaseUrl,
    /// `base_url` is not a valid absolute URL.
    #[error("invalid base_url: {0}")]
    InvalidBaseUrl(String),
    /// `model` is empty.
    #[error("model is empty")]
    EmptyModel,
    /// A non-local endpoint requires an API key.
    #[error("API key is required when base_url is not localhost")]
    MissingApiKey,
}

impl Config {
    /// Load the configuration, creating a default file when none exists.
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        if !path.exists() {
            let config = Config::default();
            config.save(path)?;
            return Ok(config);
        }
        let text = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&text)?;
        Ok(config)
    }

    /// Persist the configuration atomically with `0600` permissions.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");

        {
            let mut file = create_private_file(&tmp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        set_private(&tmp)?;
        fs::rename(&tmp, path)?;
        set_private(path)?;
        Ok(())
    }

    /// Validate the LLM section for use.
    ///
    /// A non-empty API key is required unless `base_url` points at a loopback
    /// address, so the user gets a clear message instead of a server-side 401.
    pub fn validate_llm(&self) -> Result<(), LlmConfigError> {
        let base = self.llm.base_url.trim();
        if base.is_empty() {
            return Err(LlmConfigError::EmptyBaseUrl);
        }
        let url = Url::parse(base).map_err(|e| LlmConfigError::InvalidBaseUrl(e.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| LlmConfigError::InvalidBaseUrl("missing host".to_string()))?;
        if self.llm.model.trim().is_empty() {
            return Err(LlmConfigError::EmptyModel);
        }
        if self.llm.api_key.trim().is_empty() && !is_loopback_host(host) {
            return Err(LlmConfigError::MissingApiKey);
        }
        Ok(())
    }
}

/// Normalise a base URL to the chat-completions endpoint.
///
/// Trims trailing slashes and appends `/chat/completions` unless the caller
/// already supplied the full path.
pub fn normalize_base_url(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
}
