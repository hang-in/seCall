mod common;

use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use serde_json::json;
use tokio::sync::Mutex as TokioMutex;

use common::send_request;

// tokio::sync::Mutex 사용 (clippy::await_holding_lock 회피).
static ENV_MUTEX: TokioMutex<()> = TokioMutex::const_new(());

fn write_config(path: &std::path::Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
    std::fs::write(path, body).expect("write config");
}

fn make_router(dir: &tempfile::TempDir, allow_config_edit: bool) -> axum::Router {
    let db_path = dir.path().join("test.db");
    let db = secall_core::store::Database::open(&db_path).expect("open db");
    let db_arc = Arc::new(Mutex::new(db));
    let executor = Arc::new(secall_core::jobs::JobExecutor::with_adapters(
        db_arc.clone(),
        common::make_fake_adapters(0),
    ));

    let tok = secall_core::search::LinderaKoTokenizer::new().expect("tokenizer init");
    let engine = secall_core::search::SearchEngine::new(
        secall_core::search::Bm25Indexer::new(Box::new(tok)),
        None,
    );
    let vault_path = dir.path().join("vault");
    let server = secall_core::mcp::SeCallMcpServer::new_with_options(
        db_arc,
        Arc::new(engine),
        vault_path,
        allow_config_edit,
    );
    secall_core::mcp::rest::rest_router(server, executor)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_config_masks_secret_and_reports_env_indicators() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[graph]
cloud_api_key = "secret-key"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);
    std::env::set_var("ANTHROPIC_API_KEY", "set-for-test");

    let router = make_router(&dir, false);
    let (status, body) = send_request(&router, Method::GET, "/api/config", None).await;

    std::env::remove_var("SECALL_CONFIG_PATH");
    std::env::remove_var("ANTHROPIC_API_KEY");

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert_eq!(body["graph"]["cloud_api_key"], "<masked>");
    assert_eq!(body["env_indicators"]["ANTHROPIC_API_KEY"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_config_updates_section_when_enabled() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/wiki",
        Some(json!({ "default_backend": "haiku" })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert_eq!(body["wiki"]["default_backend"], "haiku");

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(saved.contains("default_backend = \"haiku\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_config_returns_403_when_disabled() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, false);
    let (status, body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/wiki",
        Some(json!({ "default_backend": "haiku" })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "expected 403, got {status}: {body}"
    );
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("config edit disabled"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_config_unknown_section_returns_404() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/nope",
        Some(json!({ "default_backend": "haiku" })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "expected 404, got {status}: {body}"
    );
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("unknown config section"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_graph_section_ignores_cloud_api_key() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[graph]
cloud_api_key = "original-secret"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/graph",
        Some(json!({
            "cloud_api_key": "leaked-attacker-input",
            "ollama_model": "new-model"
        })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(saved.contains(r#"cloud_api_key = "original-secret""#));
    assert!(!saved.contains("leaked-attacker-input"));
    assert!(saved.contains(r#"ollama_model = "new-model""#));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_log_section_ignores_cloud_api_key() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[log]
cloud_api_key = "original-log-secret"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/log",
        Some(json!({
            "cloud_api_key": "leaked-attacker-input",
            "model": "new-model"
        })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(saved.contains(r#"cloud_api_key = "original-log-secret""#));
    assert!(!saved.contains("leaked-attacker-input"));
    assert!(saved.contains(r#"model = "new-model""#));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_preserves_other_sections_in_toml() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[wiki]
default_backend = "ollama"

[log]
backend = "haiku"
model = "claude-haiku-4-5-20251001"

[embedding]
backend = "ort"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/wiki",
        Some(json!({ "default_backend": "claude" })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(saved.contains(r#"default_backend = "claude""#));
    let parsed: toml::Value = toml::from_str(&saved).expect("saved config should parse");
    assert_eq!(
        parsed
            .get("log")
            .and_then(|v| v.get("backend"))
            .and_then(|v| v.as_str()),
        Some("haiku"),
        "expected [log].backend preserved, got:\n{saved}"
    );
    assert_eq!(
        parsed
            .get("log")
            .and_then(|v| v.get("model"))
            .and_then(|v| v.as_str()),
        Some("claude-haiku-4-5-20251001"),
        "expected [log].model preserved, got:\n{saved}"
    );
    assert!(
        saved.contains("[embedding]\nbackend = \"ort\""),
        "expected embedding section preserved, got:\n{saved}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_invalid_json_body_returns_400() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/wiki",
        Some(json!(["array", "not", "object"])),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "expected 400, got {status}: {body}"
    );
    assert!(body["error"]
        .as_str()
        .unwrap_or("")
        .contains("must be a JSON object"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_embedding_section_ignores_cloud_api_key() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[embedding]
cloud_api_key = "original-embed-secret"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/embedding",
        Some(json!({
            "cloud_api_key": "leaked-attacker-input",
            "backend": "ollama_cloud"
        })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(saved.contains(r#"cloud_api_key = "original-embed-secret""#));
    assert!(!saved.contains("leaked-attacker-input"));
    assert!(saved.contains(r#"backend = "ollama_cloud""#));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_config_masks_embedding_cloud_api_key() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[embedding]
cloud_api_key = "secret-embed-key"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, false);
    let (status, body) = send_request(&router, Method::GET, "/api/config", None).await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert_eq!(body["embedding"]["cloud_api_key"], "<masked>");
}

// ─── P48: embedding PATCH 정상 경로 + env_indicators 회귀 테스트 ─────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_embedding_section_updates_non_secret_fields() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[embedding]
backend = "ollama"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/embedding",
        Some(json!({
            "backend": "ollama_cloud",
            "cloud_host": "https://ollama.com",
            "cloud_model": "bge-m3",
            "pool_size": 2
        })),
    )
    .await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK, "PATCH embedding should return 200");

    let saved = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(
        saved.contains(r#"backend = "ollama_cloud""#),
        "backend should be updated"
    );
    assert!(
        saved.contains(r#"cloud_host = "https://ollama.com""#),
        "cloud_host should be saved"
    );
    assert!(
        saved.contains(r#"cloud_model = "bge-m3""#),
        "cloud_model should be saved"
    );
    assert!(saved.contains("pool_size = 2"), "pool_size should be saved");
    // cloud_api_key was not in PATCH body — must not appear in saved file
    assert!(
        !saved.contains("cloud_api_key"),
        "cloud_api_key must not appear when not patched"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_config_returns_pool_size_field() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[embedding]
pool_size = 4
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, false);
    let (status, body) = send_request(&router, Method::GET, "/api/config", None).await;

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert_eq!(
        body["embedding"]["pool_size"], 4,
        "pool_size should be returned as number 4"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_config_env_indicators_includes_ollama_cloud_api_key() {
    let _guard = ENV_MUTEX.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);
    std::env::set_var("OLLAMA_CLOUD_API_KEY", "test-indicator-key");

    let router = make_router(&dir, false);
    let (status, body) = send_request(&router, Method::GET, "/api/config", None).await;

    std::env::remove_var("SECALL_CONFIG_PATH");
    std::env::remove_var("OLLAMA_CLOUD_API_KEY");

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert_eq!(
        body["env_indicators"]["OLLAMA_CLOUD_API_KEY"], true,
        "OLLAMA_CLOUD_API_KEY set → env_indicator should be true"
    );
}
