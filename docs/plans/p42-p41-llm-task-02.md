---
type: task
plan_slug: p42-p41-llm
task_id: 02
title: semantic.rs LLM 분기 단위 테스트
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 02 — semantic.rs LLM 분기 단위 테스트

## Changed files

신규:
- `crates/secall-core/tests/semantic_backends.rs` (신규) — `extract_with_llm`,
  `extract_with_gemini` 의 backend 분기 + 모델 fallback 경로를 mockito 기반 HTTP mock 으로 검증.
- `crates/secall-core/Cargo.toml` (수정) — `[dev-dependencies]` 에 `mockito = "1"` 추가
  (다른 테스트가 이미 사용하면 생략).

수정 (가시성):
- `crates/secall-core/src/graph/semantic.rs:415` — `async fn extract_with_llm` 이 `pub(crate)` 이라면 그대로,
  `private` 이면 `pub(crate)` 또는 module-level 테스트로 호출 경로 확보. **확인 후 결정**.
- `crates/secall-core/src/graph/semantic.rs:273` — `extract_with_gemini` 동일. 본 task 는
  가시성을 최소한으로만 확장 (preferably `pub(crate)`).

> **사전 확인 필수**: `cargo expand` 또는 grep 으로 두 함수의 현재 가시성을 확인 후
> 가시성 변경이 필요한지 판단. 가시성 확장 없이 가능하면 `#[cfg(test)] mod tests` 안에서
> super::extract_with_llm 형태로 호출 (integration test 가 아닌 unit test 로 작성).

## Change description

### 1. 테스트 전략 결정

두 함수는 외부 HTTP (Ollama, Anthropic, Gemini, OpenAI-compat) 호출을 포함.
mockito 로 HTTP mock 을 띄우고 `GraphConfig::ollama_url` / `gemini_api_key` 를
mock URL 로 override. Anthropic / Gemini 는 `MOCK_*_BASE_URL` 같은 env var 가
필요할 수 있음 — 그 경우 본 task 에서 backend impl (extract_with_anthropic 등)
의 base URL 가 env override 가능한지 먼저 확인.

**대안 (env override 없는 경우)**: 본 task 는 ollama / lmstudio 분기만 mock 하고,
anthropic / gemini 는 향후 별도 plan (env override 도입 후) 으로 미룸.
README 에 "anthropic / gemini 는 통합 테스트 대상 외" 명시.

### 2. 테스트 케이스 (ollama / lmstudio 우선)

`tests/semantic_backends.rs`:

```rust
#[tokio::test]
async fn extract_with_llm_ollama_uses_config_model() {
    let mut server = mockito::Server::new_async().await;
    let mock = server.mock("POST", "/api/generate")
        .with_status(200)
        .with_body(json!({"response": "[]"}).to_string())
        .create_async()
        .await;

    let mut config = GraphConfig::default();
    config.semantic_backend = "ollama".into();
    config.ollama_url = Some(server.url());
    config.ollama_model = Some("custom-model".into());

    let fm = test_frontmatter();
    let edges = extract_with_llm(&config, &fm, "body").await.unwrap();
    assert_eq!(edges.len(), 0);
    mock.assert_async().await;
    // request body 가 "custom-model" 포함하는지 검증 (mockito match_body)
}

#[tokio::test]
async fn extract_with_llm_ollama_falls_back_to_default_model() {
    // ollama_model = None → GRAPH_OLLAMA_DEFAULT 사용
    // mockito match_body 로 model name 확인
}

#[tokio::test]
async fn extract_with_llm_lmstudio_uses_lmstudio_default() {
    // semantic_backend = "lmstudio", ollama_model = None
    // → GRAPH_LMSTUDIO_DEFAULT ("gemma-4-e4b-it") 사용 검증
}

#[tokio::test]
async fn extract_with_llm_unknown_backend_errors() {
    let mut config = GraphConfig::default();
    config.semantic_backend = "nonsense".into();
    let err = extract_with_llm(&config, &test_frontmatter(), "body")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("unknown semantic_backend"));
}
```

helper `test_frontmatter()` 는 최소 필드 (`session_id`, `agent`, `project`,
`turn_count`) 만 채운 dummy `SessionFrontmatter` 생성.

### 3. gemini 분기 (env-aware)

`extract_with_gemini` 는 hardcoded `https://generativelanguage.googleapis.com`
URL 사용 (P41 task 01 의 log.rs 와 동일 패턴). env override 없으면 본 task 의 mock 대상 제외.
대신 model fallback 만 검증:

- `gemini_model = None` 이고 `gemini_api_key = "fake"` 로 호출 시 mock 없이도
  request build 단계까지 진행 후 reqwest 단계에서 실패 — request body 의 URL 에
  `gemini-2.5-flash` 포함 검증을 위한 별도 helper 필요 (URL 만 build 하는
  pure function 분리). 본 task 는 그 분리는 미루고, **fallback warning trip 검증**
  으로 한정:

```rust
#[test]
fn warn_using_default_warns_only_once_per_field() {
    use secall_core::llm::defaults::{warn_using_default, GRAPH_GEMINI_DEFAULT};
    // tracing-test 또는 tracing_subscriber 의 fmt::TestWriter 로 warn 캡처
    warn_using_default("graph.gemini_model", GRAPH_GEMINI_DEFAULT);
    warn_using_default("graph.gemini_model", GRAPH_GEMINI_DEFAULT);
    // 두 번 호출해도 warn 은 1회만 — captured logs 길이 검증
}
```

> `tracing-test` crate 가 dev-dep 에 없으면 도입. 또는 `tracing_subscriber` 의
> `fmt::layer().with_writer(...)` 로 buffer 캡처. 어느 쪽이든 dev-dep only.

### 4. dev-dep 추가

`crates/secall-core/Cargo.toml`:

```toml
[dev-dependencies]
mockito = "1"
tracing-test = "0.2"   # OR equivalent
```

다른 테스트가 이미 mockito 사용 중이면 line 추가 X.

## Dependencies

- 의존 task 없음. P41 의 `defaults.rs` + `extract_with_*` 함수가 이미 존재.
- crate dep: `mockito`, optionally `tracing-test`. 추가 후 `cargo build -p secall-core --tests`.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core --tests
cargo clippy -p secall-core --tests

# 2. 신규 테스트 실행
cargo test -p secall-core --test semantic_backends

# 3. (regression) 기존 테스트 영향 없음
cargo test -p secall-core --lib graph::

# 4. (manual) test gap reduction 확인 — code-review-graph 재실행
# (CI 의 code-review-graph 단계가 다음 PR 에서 trip)
```

## Risks

- **함수 가시성 확장** — `extract_with_llm` 을 `pub(crate)` 으로 바꾸면 다른 모듈에서도 호출 가능.
  mitigation: `#[cfg(any(test, doc))]` gate 또는 doc comment 의 "test only" 명시.
- **HTTP mock 의 `localhost` race** — mockito 가 random port 사용 → 병렬 안전.
  단, `GraphConfig` 에 `ollama_url` 가 process-global state 면 race. 현재 config 는 per-call 이라 OK.
- **anthropic / gemini base URL hard-code** — 본 task 의 mock 적용 불가.
  follow-up plan 에서 `ANTHROPIC_API_BASE` / `GEMINI_API_BASE` env override 도입 검토.
- **fake frontmatter 의 분기 영향** — `extract_with_llm` 이 fm 의 어떤 필드를 prompt 에 사용하는지 확인 후 helper 작성. 현재 `build_user_content(fm, body)` 가 wrapper.
- **flaky test** — mockito 의 `expect_at_least` / `expect_at_most` 사용 권장.

## Scope boundary (수정 금지)

- `crates/secall/src/commands/log.rs` — task 01 영역.
- `crates/secall-core/src/llm/defaults.rs` — 상수 변경 X.
- `crates/secall-core/src/graph/semantic.rs` 의 함수 본문 — 가시성 외 변경 금지.
- `crates/secall-core/src/mcp/` — task 05 영역.
- `crates/secall/src/commands/config.rs` — task 03 영역.
- `web/` — task 04 영역.
