//! Integration tests for the HTTP LLM client against a mock server.

use std::time::Duration;

use dcards::config::normalize_base_url;
use dcards::llm::{HttpLlmClient, LlmClient, LlmError, LlmRequest};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn request(url: String, api_key: &str, timeout: Duration) -> LlmRequest {
    LlmRequest {
        url,
        system_prompt: "You are a dictionary.".to_string(),
        user_message: "hello".to_string(),
        model: "test-model".to_string(),
        max_tokens: 80,
        temperature: 0.2,
        timeout,
        api_key: api_key.to_string(),
    }
}

/// A successful chat-completion body wrapping `content`.
fn chat_body(content: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "choices": [
            { "message": { "role": "assistant", "content": content } }
        ]
    }))
}

async fn mount_chat(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn happy_path_returns_content() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("сердце"))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let answer = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap();

    assert_eq!(answer, "сердце");
}

#[tokio::test]
async fn surrounding_quotes_are_stripped() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("\"Hello world\""))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let answer = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap();

    assert_eq!(answer, "Hello world");
}

#[tokio::test]
async fn empty_content_is_an_error() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!(""))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let err = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap_err();

    assert_eq!(err, LlmError::EmptyResponse);
}

#[tokio::test]
async fn null_content_is_an_error() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!(null))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let err = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap_err();

    assert_eq!(err, LlmError::EmptyResponse);
}

#[tokio::test]
async fn server_error_is_mapped_to_http() {
    let server = MockServer::start().await;
    mount_chat(&server, ResponseTemplate::new(500).set_body_string("boom")).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let err = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap_err();

    match err {
        LlmError::Http {
            status,
            body_snippet,
        } => {
            assert_eq!(status, 500);
            assert_eq!(body_snippet, "boom");
        }
        other => panic!("expected Http, got {other:?}"),
    }
}

#[tokio::test]
async fn malformed_json_is_a_parse_error() {
    let server = MockServer::start().await;
    mount_chat(
        &server,
        ResponseTemplate::new(200).set_body_string("not json"),
    )
    .await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let err = client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap_err();

    assert!(matches!(err, LlmError::Parse(_)), "got {err:?}");
}

#[tokio::test]
async fn timeout_is_reported() {
    let server = MockServer::start().await;
    mount_chat(
        &server,
        chat_body(json!("late")).set_delay(Duration::from_millis(400)),
    )
    .await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    let err = client
        .complete(request(url, "", Duration::from_millis(50)))
        .await
        .unwrap_err();

    assert_eq!(err, LlmError::Timeout);
}

#[tokio::test]
async fn authorization_header_is_sent_when_key_is_present() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("ok"))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    client
        .complete(request(url, "secret-key", Duration::from_secs(5)))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let authorization = requests[0]
        .headers
        .get("authorization")
        .expect("authorization header should be present")
        .to_str()
        .unwrap();
    assert_eq!(authorization, "Bearer secret-key");
}

#[tokio::test]
async fn authorization_header_is_trimmed() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("ok"))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    client
        .complete(request(
            url,
            "  \"Bearer secret-key\"\n",
            Duration::from_secs(5),
        ))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let authorization = requests[0]
        .headers
        .get("authorization")
        .expect("authorization header should be present")
        .to_str()
        .unwrap();
    assert_eq!(authorization, "Bearer secret-key");
}

#[tokio::test]
async fn authorization_header_is_omitted_without_key() {
    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("ok"))).await;

    let client = HttpLlmClient::new().unwrap();
    let url = format!("{}/v1/chat/completions", server.uri());
    client
        .complete(request(url, "", Duration::from_secs(5)))
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("authorization").is_none());
}

#[tokio::test]
async fn request_built_from_config_uses_the_local_endpoint() {
    use dcards::db::LangPair;
    use dcards::llm::build_request;

    let server = MockServer::start().await;
    mount_chat(&server, chat_body(json!("дом"))).await;

    let mut config = dcards::config::Config::default();
    config.llm.base_url = format!("{}/v1", server.uri());
    config.llm.api_key = String::new();
    config.llm.model = "test-model".to_string();

    let request = build_request("house", LangPair::EnRu, &config).unwrap();
    assert_eq!(request.system_prompt, config.prompts.en_ru);

    let answer = HttpLlmClient::new()
        .unwrap()
        .complete(request)
        .await
        .unwrap();
    assert_eq!(answer, "дом");
}

#[test]
fn remote_endpoint_requires_an_api_key() {
    use dcards::db::LangPair;
    use dcards::llm::build_request;

    let mut config = dcards::config::Config::default();
    config.llm.base_url = "https://api.example.com/v1".to_string();
    config.llm.api_key = String::new();

    assert!(matches!(
        build_request("house", LangPair::EnRu, &config),
        Err(LlmError::MissingApiKey)
    ));
}

#[test]
fn normalize_base_url_builds_the_endpoint() {
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1"),
        "http://localhost:11434/v1/chat/completions"
    );
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1/"),
        "http://localhost:11434/v1/chat/completions"
    );
    // Already a full path: must not be duplicated.
    assert_eq!(
        normalize_base_url("http://localhost:11434/v1/chat/completions"),
        "http://localhost:11434/v1/chat/completions"
    );
}
