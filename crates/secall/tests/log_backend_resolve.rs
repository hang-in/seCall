use secall::commands::log::{generate_log_body, resolve_backend_name, resolve_log_model};
use secall_core::{
    llm::defaults::{LOG_OLLAMA_CLOUD_DEFAULT, LOG_OLLAMA_DEFAULT},
    vault::Config,
};
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[test]
fn backend_resolution_priority_matches_plan() {
    let mut config = Config::default();
    config.log.backend = Some("claude".to_string());
    config.graph.semantic_backend = "ollama_cloud".to_string();
    assert_eq!(resolve_backend_name(&config, Some("haiku")), "haiku");
    assert_eq!(resolve_backend_name(&config, None), "claude");

    config.log.backend = None;
    assert_eq!(resolve_backend_name(&config, None), "ollama_cloud");

    config.graph.semantic_backend.clear();
    assert_eq!(resolve_backend_name(&config, None), "ollama");
}

#[test]
fn model_resolution_priority_matches_plan() {
    let mut config = Config::default();
    config.log.model = Some("config-model".to_string());

    assert_eq!(
        resolve_log_model(&config, "ollama", Some("cli-model")).as_deref(),
        Some("cli-model")
    );
    assert_eq!(
        resolve_log_model(&config, "ollama", None).as_deref(),
        Some("config-model")
    );

    config.log.model = None;
    assert_eq!(
        resolve_log_model(&config, "ollama", None).as_deref(),
        Some(LOG_OLLAMA_DEFAULT)
    );
    assert_eq!(resolve_log_model(&config, "claude", None), None);
    assert_eq!(resolve_log_model(&config, "codex", None), None);
    assert_eq!(resolve_log_model(&config, "lmstudio", None), None);
}

#[test]
fn model_resolution_ollama_cloud_uses_cloud_model() {
    let mut config = Config::default();
    config.log.cloud_model = Some("kimi-k2.6:cloud".to_string());

    assert_eq!(
        resolve_log_model(&config, "ollama_cloud", None).as_deref(),
        Some("kimi-k2.6:cloud")
    );

    // log.cloud_model 없으면 graph.cloud_model 폴백
    config.log.cloud_model = None;
    config.graph.cloud_model = Some("gemma4:31b-cloud".to_string());
    assert_eq!(
        resolve_log_model(&config, "ollama_cloud", None).as_deref(),
        Some("gemma4:31b-cloud")
    );

    // 둘 다 None → LOG_OLLAMA_CLOUD_DEFAULT
    config.log.cloud_model = None;
    config.graph.cloud_model = None;
    assert_eq!(
        resolve_log_model(&config, "ollama_cloud", None).as_deref(),
        Some(LOG_OLLAMA_CLOUD_DEFAULT)
    );
}

#[tokio::test]
async fn ollama_cloud_api_key_env_sets_both_graph_and_log() {
    let _guard = ENV_LOCK.lock().await;
    std::env::set_var("OLLAMA_CLOUD_API_KEY", "test-cloud-key-xyz");

    let config = Config::load_or_default();

    assert_eq!(
        config.graph.cloud_api_key.as_deref(),
        Some("test-cloud-key-xyz"),
        "OLLAMA_CLOUD_API_KEY must propagate to graph.cloud_api_key"
    );
    assert_eq!(
        config.log.cloud_api_key.as_deref(),
        Some("test-cloud-key-xyz"),
        "OLLAMA_CLOUD_API_KEY must propagate to log.cloud_api_key"
    );

    std::env::remove_var("OLLAMA_CLOUD_API_KEY");
}

// ─── P48: generate_log_body ollama_cloud arm 통합 회귀 테스트 ────────────────

fn ollama_generate_response() -> String {
    serde_json::json!({ "response": "생성된 일기 내용" }).to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generate_log_body_ollama_cloud_includes_bearer_auth() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/api/generate")
        .match_header("Authorization", "Bearer env-key")
        .with_status(200)
        .with_body(ollama_generate_response())
        .create_async()
        .await;

    let mut config = secall_core::vault::Config::default();
    config.log.cloud_host = Some(server.url());
    config.log.cloud_api_key = Some("env-key".to_string());
    config.log.cloud_model = Some("kimi-k2.6:cloud".to_string());

    let result = generate_log_body(
        &config,
        Some("ollama_cloud"),
        None,
        "system prompt",
        "user prompt",
        "2026-05-12",
    )
    .await;
    assert!(
        result.is_ok(),
        "generate_log_body should succeed: {:?}",
        result.err()
    );
    mock.assert_async().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generate_log_body_ollama_cloud_uses_resolve_log_model_chain() {
    let mut server = mockito::Server::new_async().await;

    // log.cloud_model 없고 graph.cloud_model 만 있는 경우
    let mock = server
        .mock("POST", "/api/generate")
        .match_body(mockito::Matcher::Regex(
            r#""model":"g-cloud-model""#.to_string(),
        ))
        .with_status(200)
        .with_body(ollama_generate_response())
        .create_async()
        .await;

    let mut config = secall_core::vault::Config::default();
    config.log.cloud_host = Some(server.url());
    config.log.cloud_api_key = Some("test-key".to_string());
    config.log.cloud_model = None;
    config.graph.cloud_model = Some("g-cloud-model".to_string());

    let result = generate_log_body(
        &config,
        Some("ollama_cloud"),
        None,
        "system prompt",
        "user prompt",
        "2026-05-12",
    )
    .await;
    assert!(
        result.is_ok(),
        "should use graph.cloud_model: {:?}",
        result.err()
    );
    mock.assert_async().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generate_log_body_ollama_cloud_missing_api_key_returns_err() {
    let mut config = secall_core::vault::Config::default();
    config.log.cloud_host = Some("https://ollama.com".to_string());
    config.log.cloud_api_key = None;
    config.graph.cloud_api_key = None;

    let result = generate_log_body(
        &config,
        Some("ollama_cloud"),
        None,
        "system prompt",
        "user prompt",
        "2026-05-12",
    )
    .await;
    assert!(result.is_err(), "missing api_key should return Err");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("OLLAMA_CLOUD_API_KEY")
            || msg.contains("cloud_api_key")
            || msg.contains("api key"),
        "error message should mention missing key, got: {msg}"
    );
}
