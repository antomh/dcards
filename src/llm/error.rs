//! Errors produced by the LLM client.

/// Everything that can go wrong while talking to the LLM.
///
/// The API key is never included in any variant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlmError {
    /// The endpoint is remote but no API key is configured.
    #[error("API key is required when base_url is not localhost")]
    MissingApiKey,
    /// The LLM section of the configuration is not usable.
    #[error("invalid LLM configuration: {0}")]
    InvalidConfig(String),
    /// A transport-level failure (DNS, connection, TLS, ...).
    #[error("network error: {0}")]
    Network(String),
    /// The request exceeded its timeout.
    #[error("request timed out")]
    Timeout,
    /// The server answered with a non-success status.
    #[error("HTTP {status}: {body_snippet}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// First 256 characters of the response body.
        body_snippet: String,
    },
    /// The response body could not be parsed.
    #[error("failed to parse the response: {0}")]
    Parse(String),
    /// The response contained no usable text.
    #[error("the model returned an empty response")]
    EmptyResponse,
    /// The model reported that it could not translate or define the word.
    #[error("translation unavailable")]
    TranslationUnavailable,
}
