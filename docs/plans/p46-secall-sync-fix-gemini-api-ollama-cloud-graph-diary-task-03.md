---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P46
task_id: 03
parallel_group: 2
depends_on: [02]
---

# P46 Task 03 — Ollama Cloud 백엔드 도입

## Changed files

### Config schema 확장

- `crates/secall-core/src/vault/config.rs:158-189` — `GraphConfig` 에 cloud 옵션 추가:
  - `pub cloud_host: Option<String>` (기본 `"https://ollama.com"`)
  - `pub cloud_model: Option<String>` (defaults 는 Task 04 에서 설정)
  - `pub cloud_api_key: Option<String>` (config 값, env 우선)
- `crates/secall-core/src/vault/config.rs:191-202` — `LogConfig` 에 동일 cloud 옵션 추가:
  - `pub cloud_host: Option<String>`
  - `pub cloud_model: Option<String>`
  - `pub cloud_api_key: Option<String>`
- `crates/secall-core/src/vault/config.rs:163` — `semantic_backend` doc comment 갱신: `"ollama" (기본) | "anthropic" | "ollama_cloud" | "lmstudio" | "disabled"`
- `crates/secall-core/src/vault/config.rs:362-380` — env override 섹션에 `OLLAMA_CLOUD_API_KEY` 처리 추가. 우선순위: config field → env. 이 env 는 graph 와 log 모두에서 같은 키를 공유 (사용자 .env 에 하나만 둠).

### Graph dispatch 추가

- `crates/secall-core/src/graph/semantic.rs:420-454` — `extract_with_llm` 의 match arm 에 `"ollama_cloud"` 추가. 구현은 기존 `extract_with_ollama` 와 거의 동일하되:
  - base URL = `cloud_host` (기본 `https://ollama.com`)
  - HTTP request 헤더에 `Authorization: Bearer <api_key>` 추가
  - 나머지 (`/api/chat`, request/response 스키마) 는 동일.
- `crates/secall-core/src/graph/semantic.rs` 어딘가 (예: `extract_with_ollama` 바로 아래) — 새 함수 `extract_with_ollama_cloud(fm, body, base_url, model, api_key) -> Result<Vec<GraphEdge>>` 추가. 기존 `extract_with_ollama` 시그니처 변경 X (회귀 방지).

### Log dispatch 추가

- `crates/secall/src/commands/log.rs:246-324` — `generate_log_body` 의 `match backend_name.as_str()` 에 `"ollama_cloud"` arm 추가. `OllamaBackend` 의 cloud 변형을 만들거나, `OllamaBackend` 자체에 `api_key: Option<String>` 필드를 추가하고 cloud 모드는 host=`https://ollama.com` + api_key=Some(...) 으로 식별.

  **권장 접근: `OllamaBackend` 확장** — `crates/secall-core/src/wiki/ollama.rs:5` 의 `pub struct OllamaBackend` 에 `pub api_key: Option<String>` 필드 추가. `generate` 구현에서 `api_key.is_some()` 이면 `Authorization: Bearer ...` 헤더 첨부. 이 방식이 코드 중복 최소화.

- `crates/secall/src/commands/log.rs:170-182` — `resolve_backend_name` 에는 변경 없음 (string 값만 새 식별자 사용).
- `crates/secall/src/commands/log.rs:185-209` — `resolve_log_model` 에 `"ollama_cloud" =>` arm 추가. config `[log].cloud_model` 또는 `[graph].cloud_model` 에서 가져옴. defaults 는 Task 04 에서.
- `crates/secall/src/commands/log.rs:211-216` — `resolve_log_api_url` 에 `"ollama_cloud"` arm 추가. config `[log].cloud_host` → `[graph].cloud_host` → `"https://ollama.com"` 폴백.

### Wiki backend 확장 (cloud 헤더 지원)

- `crates/secall-core/src/wiki/ollama.rs:5-` — `OllamaBackend` 에 `pub api_key: Option<String>` 필드 추가. `generate` 구현에서 reqwest builder 에 `.bearer_auth(&key)` 또는 헤더 추가. 기존 호출자 (`wiki.rs:635` 등) 는 `api_key: None` 로 명시.

### Config test (secall config test)

- `crates/secall/src/commands/config.rs:122-135` — `run_llm_test` 의 backends 리스트에 `"ollama_cloud"` 추가.
- `crates/secall/src/commands/config.rs` 어딘가 (`test_ollama_backend` 부근) — 신규 함수 `test_ollama_cloud_backend(config, no_network) -> TestOutcome` 추가. config 의 `cloud_api_key` 또는 `OLLAMA_CLOUD_API_KEY` env 존재 여부 확인 + (network 모드면) `cloud_host` 의 `/api/tags` HEAD 호출 등.

### main.rs dotenv 로딩

- `crates/secall/src/main.rs:454` — dotenv 주석에서 `SECALL_GEMINI_API_KEY` → `OLLAMA_CLOUD_API_KEY` 또는 통합 표현으로 갱신 (실제 dotenv 동작은 변경 없음).

### 단위 테스트

- `crates/secall-core/src/graph/semantic.rs` `#[cfg(test)]` — `extract_with_ollama_cloud` 가 `cloud_api_key` 없을 때 명확한 에러 (`anyhow!("ollama cloud api key not set ...")`) 반환하는지 (network 호출 전) 검증하는 테스트 추가.
- `crates/secall-core/src/vault/config.rs` `#[cfg(test)]` — `OLLAMA_CLOUD_API_KEY` env override 가 `graph.cloud_api_key` / `log.cloud_api_key` 에 반영되는지 + ENV_MUTEX 잠금 사용.

## Change description

### 단계별 접근

1. **Config schema 확장 먼저** — `GraphConfig` / `LogConfig` 에 cloud 필드 3개씩 추가. `#[serde(default)]` 로 backwards-compatible.

2. **env override** — `Config::with_env_overrides` (또는 동등 메서드) 에서 `OLLAMA_CLOUD_API_KEY` env 가 있으면 `graph.cloud_api_key` 와 `log.cloud_api_key` 둘 다 `Some(value)` 로 설정. 둘 다 사용하는 곳이 다르므로 분리.

3. **Graph dispatch** — `semantic.rs:441` 의 `match` 에 `"ollama_cloud" =>` arm 추가. 구현은 `extract_with_ollama_cloud` 라는 새 함수에 위임. 이 함수는 base_url + Bearer 헤더만 차이.

4. **Log dispatch** — `OllamaBackend` 에 `api_key: Option<String>` 추가. `commands/log.rs` 의 `match backend_name.as_str()` 에 `"ollama_cloud" =>` arm 추가:
   ```text
   "ollama_cloud" => {
       let api_url = ... cloud_host or "https://ollama.com" ...;
       let model = ... cloud_model ...;
       let api_key = config.log.cloud_api_key (or graph.cloud_api_key) or env...;
       let backend = OllamaBackend { api_url, model, max_tokens, api_key: Some(api_key) };
       backend.generate(...).await
   }
   ```

5. **Wiki backend (`OllamaBackend`) 확장** — 필드 추가 + Authorization header 첨부. 기존 호출부 `crates/secall/src/commands/wiki.rs:635` 의 `config.wiki_backend_config("ollama")` 가 `OllamaBackend` 를 만들 때 `api_key: None` 명시.

6. **config test 추가** — `secall config test ollama_cloud` 가 동작하도록 dispatch 와 함수 추가.

### 구현 제약

- **단일 OllamaBackend struct 재사용** — 별도의 `OllamaCloudBackend` 를 만들면 코드 중복. `OllamaBackend.api_key: Option<String>` 으로 분기.
- **env 우선순위 명시** — config `cloud_api_key` field → `OLLAMA_CLOUD_API_KEY` env → 에러. fallback 텍스트는 "ollama cloud api key not set (set `OLLAMA_CLOUD_API_KEY` env or `[graph]/[log].cloud_api_key` in config.toml)".
- **`cloud_host` 의 trailing slash 처리** — `base_url.trim_end_matches('/')` 패턴 따라 처리 (기존 ollama 코드와 동일).
- **타임아웃** — cloud 는 latency 가 local 보다 길 수 있음. 기존 `Duration::from_secs(120)` 유지하되 Task 04 에서 diary 가드 적용 후에도 충분한지 검토.

## Dependencies

- **Task 02 완료 필수** — Gemini 제거 후 dispatch arm 깨끗한 상태에서 ollama_cloud 추가.

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 단위 테스트 — config schema, env override, dispatch
cargo test -p secall-core --lib vault::config
cargo test -p secall-core --lib graph::semantic
cargo test -p secall-core --lib wiki::ollama
cargo test -p secall --lib commands::log
cargo test -p secall --lib commands::config

# 통합 테스트 — backend resolve
cargo test -p secall --test log_backend_resolve

# Manual smoke test: OLLAMA_CLOUD_API_KEY 설정 후 dispatch 동작 확인 (network 필요)
# OLLAMA_CLOUD_API_KEY=$(grep OLLAMA_CLOUD_API_KEY .env | cut -d= -f2) \
#   cargo run -p secall -- config test ollama_cloud
# Manual: 출력에 [ollama_cloud] OK 또는 명확한 SKIP 사유가 나와야 함
```

## Risks

- **Ollama Cloud API 호환성** — Ollama Cloud 가 `/api/chat` 엔드포인트와 OpenAI 호환 chat 포맷을 그대로 받는지 사용자가 직접 사용해보고 확인 필요. 만약 다르다면 `extract_with_openai_compat` 패턴으로 분기 변경 (Task 05 manual smoke 에서 발견 시 follow-up).
- **OllamaBackend 시그니처 변경 파급** — `wiki.rs:635` 와 `log.rs:282-291`, 그리고 wiki review pipeline 등 `OllamaBackend` 를 직접 생성하는 모든 곳을 `api_key: None` 으로 갱신해야 함. grep 으로 확인:
  ```bash
  grep -rn "OllamaBackend {" crates/ --include='*.rs'
  ```
- **`cloud_api_key` 가 토큰화/덤프 시 노출** — config save 시 `cloud_api_key` 가 `config.toml` 에 평문으로 기록되면 사용자 보안 위험. **권장: config 에는 키를 저장하지 않고 env 만 사용** — Task 05 문서에 명시.
- **REST `/api/config` 노출** — `crates/secall-core/tests/rest_config.rs` 에 `cloud_api_key` 가 REST PATCH 로 새지 않게 가드 필요 (Task 05 또는 이 Task 의 dispatch 작업과 함께).
- code-review-graph 영향: `OllamaBackend` 시그니처 변경은 21+ 호출자에 영향 가능. 모두 `api_key: None` 으로 채우는 단순 변경이지만 누락 시 컴파일 실패로 즉시 잡힘.

## Scope boundary (수정 금지)

- `crates/secall-core/src/ingest/gemini.rs`, `gemini_web.rs`, `detect.rs::find_gemini_sessions`.
- `crates/secall/src/commands/sync.rs` — Task 01 영역.
- Default 모델 값 (Task 04 영역) — 이 Task 에서는 default 미설정 시 명확한 에러만 반환.
- diary 입력 길이 가드 (Task 04 영역).
- 문서 갱신, web SettingsRoute UI 갱신 (Task 05 영역).
