---
type: task
plan_slug: p42-p41-llm
task_id: 05
title: REST `/api/config` 추가 회귀
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 05 — REST `/api/config` 추가 회귀

## Changed files

수정:
- `crates/secall-core/tests/rest_config.rs` — 기존 4 case 외 신규 3 case 추가:
  1. `test_patch_graph_section_ignores_gemini_api_key`
  2. `test_patch_preserves_other_sections_in_toml`
  3. `test_patch_invalid_json_body_returns_400`

신규: 없음 (기존 파일에 case 추가만).

> 가능하면 P41 의 기존 4 case (`test_get_config_masks_secret_and_reports_env_indicators`,
> `test_patch_config_updates_section_when_enabled`, `test_patch_config_returns_403_when_disabled`,
> `test_patch_config_unknown_section_returns_404`) 와 동일 helper (`make_router`, `write_config`,
> `send_request`, `ENV_MUTEX`) 재사용.

## Change description

### 1. graph 섹션 PATCH 의 `gemini_api_key` 무시 회귀

P41 의 `do_config_patch` (`crates/secall-core/src/mcp/server.rs:347-350`) 가
graph 섹션 patch 시 `gemini_api_key` 키를 무시하도록 구현됨. 회귀 테스트 부재.

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_graph_section_ignores_gemini_api_key() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config").join("config.toml");
    write_config(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[graph]
gemini_api_key = "original-secret"
"#,
    );
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    let (status, _body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/graph",
        Some(json!({
            "gemini_api_key": "leaked-attacker-input",
            "ollama_model": "new-model"
        })),
    ).await;
    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    // 디스크 toml 검증
    let saved = std::fs::read_to_string(&config_path).unwrap();
    // gemini_api_key 는 원본 유지
    assert!(saved.contains(r#"gemini_api_key = "original-secret""#),
        "expected original gemini_api_key preserved, got:\n{saved}");
    assert!(!saved.contains("leaked-attacker-input"),
        "attacker payload leaked into config:\n{saved}");
    // ollama_model 은 갱신
    assert!(saved.contains(r#"ollama_model = "new-model""#));
}
```

### 2. 다른 섹션 보존 회귀

PATCH 가 한 섹션만 수정 → 다른 섹션은 디스크 toml 에 그대로 보존되는지 검증.

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_preserves_other_sections_in_toml() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
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
        Some(json!({"default_backend": "claude"})),
    ).await;
    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::OK);

    let saved = std::fs::read_to_string(&config_path).unwrap();
    // wiki 섹션 갱신
    assert!(saved.contains(r#"default_backend = "claude""#));
    // log 섹션 보존
    assert!(saved.contains(r#"backend = "haiku""#));
    assert!(saved.contains(r#"model = "claude-haiku-4-5-20251001""#));
    // embedding 섹션 보존
    assert!(saved.contains(r#"backend = "ort""#));
}
```

### 3. 잘못된 JSON body → 400

P41 의 `api_config_patch` 가 body 가 JSON object 가 아닐 때 400 을 반환.

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_patch_invalid_json_body_returns_400() {
    let _guard = ENV_MUTEX.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config").join("config.toml");
    write_config(&config_path, r#"
[vault]
path = "/tmp/test-vault"
"#);
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let router = make_router(&dir, true);
    // body 가 array — `as_object()` 가 None → "config patch body must be a JSON object"
    let (status, body) = send_request(
        &router,
        Method::PATCH,
        "/api/config/wiki",
        Some(json!(["array", "not", "object"])),
    ).await;
    std::env::remove_var("SECALL_CONFIG_PATH");

    assert_eq!(status, StatusCode::BAD_REQUEST,
        "expected 400, got {status}: {body}");
    assert!(body["error"].as_str().unwrap_or("").contains("must be a JSON object"));
}
```

> **검증 필요**: axum 의 `Json<serde_json::Value>` extractor 가 JSON array 를 받아 들이는지.
> array 를 거부하고 4xx 면 본 case 의 의도가 안 맞음 — 그 경우 body 를 `{"foo": "bar"}` 같은
> object 인데 invalid TOML 로 직렬화되는 케이스로 변경 (예: `null`, `nested object` 만 valid).
> 또는 string body (`"not an object"`) 를 보내고 400 검증.

### 4. 회귀 case 통합

기존 4 case + 신규 3 case = 7 case. ENV_MUTEX 로 직렬화 — race 없음.

## Dependencies

- 의존 task 없음. P41 task 03 의 endpoint 위에서 동작.
- crate dep: 추가 없음.

## Verification

```bash
# 1. 신규 + 기존 case 모두 통과
cargo test -p secall-core --test rest_config

# 2. case 수 확인 (7 개)
cargo test -p secall-core --test rest_config -- --list 2>&1 | grep ": test$" | wc -l
# 출력: 7

# 3. 특정 신규 case 격리 실행 (debug 시)
cargo test -p secall-core --test rest_config test_patch_graph_section_ignores_gemini_api_key
cargo test -p secall-core --test rest_config test_patch_preserves_other_sections_in_toml
cargo test -p secall-core --test rest_config test_patch_invalid_json_body_returns_400
```

## Risks

- **toml 직렬화 후 비교** — `Config::save()` 가 `toml::to_string_pretty` 사용.
  필드 순서가 alphabetical 또는 struct 정의 순서. 회귀 테스트는 `assert!(saved.contains(...))`
  로 substring 검증 → 순서 무관.
- **race** — ENV_MUTEX 로 SECALL_CONFIG_PATH env 직렬화. 다른 test crate 와 동시 실행 시
  global env var race 가능 — 본 plan 은 같은 binary 안에서만 실행되므로 안전.
- **json body 가 array 인 케이스** — axum 의 Json extractor 가 어떻게 처리하는지 사전 확인.
  현재 `Json<serde_json::Value>` 는 valid JSON 이면 모두 받아들임 → array 도 들어와서
  핸들러 안에서 `as_object()` None → 400. 검증 후 진행.
- **`do_config_patch` 의 graph sanitize 검증** — 본 task 의 case 1 이 P41 의 server.rs:347
  의 동작을 회귀로 묶음. graph 섹션 외 다른 섹션 (wiki / log / embedding) 의 secret 필드는
  현재 없음 — 본 case 는 graph 한정.
- **테스트 추가만** — 코드 변경 없음. 만약 case 1 이 fail 하면 P41 의 sanitize 로직에
  버그 — 그 경우 task 02 에서 fix 가 아니라 별도 hotfix subtask 로 escalate.

## Scope boundary (수정 금지)

- `crates/secall-core/src/mcp/server.rs` — 본 task 는 회귀 테스트만. 코드 변경 X.
- `crates/secall-core/src/mcp/rest.rs` — 변경 X.
- `crates/secall-core/src/vault/config.rs` — 변경 X.
- `crates/secall/src/commands/log.rs` — task 01 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 02 영역.
- `crates/secall/src/commands/config.rs` — task 03 영역.
- `web/` — task 04 영역.
