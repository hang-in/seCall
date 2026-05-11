---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P47
task_id: 03
parallel_group: 2
depends_on: [02]
---

# P47 Task 03 — OllamaEmbedder cloud 모드 지원

## Changed files

### Config schema

- `crates/secall-core/src/vault/config.rs:68-91` — `EmbeddingConfig` 에 다음 필드 3개 추가:
  ```text
  /// Ollama Cloud API host (기본: https://ollama.com)
  pub cloud_host: Option<String>,
  /// Ollama Cloud embedding 모델 (예: bge-m3 호환 모델)
  pub cloud_model: Option<String>,
  /// Ollama Cloud API key — env override 가 일반적 사용
  pub cloud_api_key: Option<String>,
  ```
- `crates/secall-core/src/vault/config.rs:368-390` — `apply_env_overrides` 의 `OLLAMA_CLOUD_API_KEY` 처리 블록 (P46 에서 graph/log 양쪽 set 하던 블록) 에 한 줄 추가:
  ```text
  self.embedding.cloud_api_key = Some(k);
  ```
  (graph/log 와 동일 키 공유 — env 한 번 설정으로 셋 다 적용)

### OllamaEmbedder 확장

- `crates/secall-core/src/search/embedding.rs:30-54` — `OllamaEmbedder` 구조체에 `pub api_key: Option<String>` 필드 추가. `new` 생성자에는 인자 추가하지 않고 (호환성), 별도 `with_api_key(self, key: Option<String>)` builder 메서드 또는 `pub fn new_cloud(base_url: &str, model: &str, api_key: String) -> Self` 헬퍼 추가.
  - **권장 접근**: builder 패턴
    ```text
    impl OllamaEmbedder {
        pub fn new(base_url: Option<&str>, model: Option<&str>) -> Self { ... api_key: None }
        pub fn with_api_key(mut self, key: Option<String>) -> Self {
            self.api_key = key;
            self
        }
    }
    ```
- `crates/secall-core/src/search/embedding.rs:65-87` — `embed_batch` 의 reqwest 호출 (`.post(format!("{}/api/embed", ...))`) 직후 `.bearer_auth(key)` 분기 추가. 헤더 첨부 패턴은 P46 의 `wiki/ollama.rs` 와 동일.
- `crates/secall-core/src/search/embedding.rs:89-95` — `is_available` 의 `/api/tags` 호출에도 동일하게 `bearer_auth` 추가 (cloud 가 인증 요구할 수 있음).

### Vector indexer dispatch

- `crates/secall-core/src/search/vector.rs:391-466` — `create_vector_indexer` 의 match 분기에 `"ollama_cloud"` arm 추가:
  ```text
  "ollama_cloud" => {
      let base_url = config.embedding.cloud_host.as_deref().unwrap_or("https://ollama.com");
      let model = config.embedding.cloud_model.as_deref().or(config.embedding.ollama_model.as_deref());
      let api_key = config.embedding.cloud_api_key.clone();
      if api_key.is_none() {
          tracing::warn!(
              "ollama_cloud embedding api key not set, falling back to local Ollama"
          );
          return try_ollama_fallback_with_ann(config).await;
      }
      let embedder = OllamaEmbedder::new(Some(base_url), model).with_api_key(api_key);
      if embedder.is_available().await {
          tracing::info!(host = base_url, "Ollama Cloud embedder ready");
          VectorIndexer::new(Box::new(embedder))
      } else {
          tracing::warn!("Ollama Cloud unreachable, falling back to local Ollama");
          return try_ollama_fallback_with_ann(config).await;
      }
  }
  ```

### REST API secret 필터링 (P46 패턴 확장)

- `crates/secall-core/src/mcp/server.rs:do_config_get` — `embedding` 섹션 응답에서 `cloud_api_key` 를 마스킹 (`"<masked>"` 처럼). graph/log 에서 이미 하던 패턴 그대로 embedding 에 확장.
- `crates/secall-core/src/mcp/server.rs:do_config_patch` `"embedding"` arm — `cloud_api_key` 필드 sanitize (P46 rework 의 graph/log 패턴 그대로).
- env_indicators 응답에 `OLLAMA_CLOUD_API_KEY` 가 이미 P46 에서 등록됐다면 변경 불필요. 미등록이면 추가.

### 단위 테스트

- `crates/secall-core/src/search/embedding.rs` `#[cfg(test)]` — `OllamaEmbedder::with_api_key(Some(...))` 결과의 reqwest 호출이 Authorization 헤더를 포함하는지 mockito 로 검증.
- `crates/secall-core/src/vault/config.rs` `#[cfg(test)]` — `OLLAMA_CLOUD_API_KEY` env 가 `embedding.cloud_api_key` 에도 propagate 되는지 회귀 (ENV_MUTEX 사용).

### Manual smoke test (Cloud 가용성 확인)

- 작업 초반에 다음 manual 명령으로 Ollama Cloud 가 embedding 을 지원하는지 1회 확인:
  ```bash
  OLLAMA_CLOUD_API_KEY=$(grep OLLAMA_CLOUD_API_KEY .env | cut -d= -f2)
  curl -sS -X POST https://ollama.com/api/embed \
    -H "Authorization: Bearer $OLLAMA_CLOUD_API_KEY" \
    -H "content-type: application/json" \
    -d '{"model":"bge-m3","input":["테스트"]}' | jq '.embeddings | length'
  ```
- 응답이 200 + `embeddings` 길이 ≥ 1 이면 진행. 4xx (모델 미지원) 이면 본 task 의 dispatch arm 은 "명확한 에러 + Cloud 미지원 안내" 로 마무리하고 ingest 사용자에게는 ollama 로컬 사용 유도.

## Change description

### 단계별 접근

1. **Manual smoke 먼저** — Ollama Cloud `/api/embed` 가용성 확인. 실패 시 dispatch arm 은 추가하되 안내 메시지로 종결.

2. **Config schema 확장** — `EmbeddingConfig` 에 3개 필드. env override 한 줄 추가.

3. **OllamaEmbedder builder 확장** — `api_key` 필드 + `with_api_key` 메서드. `embed_batch` / `is_available` 에 bearer_auth 분기.

4. **vector.rs dispatch** — `"ollama_cloud"` arm 추가. api_key 없거나 unreachable 시 로컬 Ollama 폴백 (`try_ollama_fallback_with_ann`).

5. **REST API secret 필터링 확장** — `mcp/server.rs` 의 graph/log 와 동일한 sanitize 를 embedding 섹션에 추가.

6. **단위 테스트 + Manual smoke 재실행** — `secall sync --backend ollama_cloud` (또는 config 수정 후 sync) 으로 실제 embedding 호출 확인.

### 구현 제약

- **`OllamaEmbedder::new` 시그니처 보존** — 기존 호출자 (`vector.rs:500, 564` 등) 영향 X. 새 기능은 builder 메서드로만.
- **Cloud 미지원 시 graceful fallback** — api_key 없으면 로컬 Ollama 로 자동 폴백. 사용자가 명시적으로 `ollama_cloud` 를 선택했는데 키가 없으면 시작 시 warn 로그 + 로컬 사용.
- **REST 보안** — P46 rework 에서 결정된 "모든 섹션에서 secret PATCH 차단" 원칙을 embedding 에도 동일 적용. `cloud_api_key` 는 env / `.env` 에서만 관리.

## Dependencies

- **Task 02 완료 후** — config schema 가 `pool_size` 와 함께 갱신되므로 같이 손대는 편이 안전.
- 외부: Ollama Cloud `/api/embed` 엔드포인트 가용성 (manual smoke 로 확인).

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 단위 테스트
cargo test -p secall-core --lib search::embedding
cargo test -p secall-core --lib vault::config::tests
cargo test -p secall-core --test rest_config

# Manual: cloud 가용성 확인 (network 필요)
# OLLAMA_CLOUD_API_KEY=... curl -X POST https://ollama.com/api/embed \
#   -H "Authorization: Bearer $OLLAMA_CLOUD_API_KEY" \
#   -H "content-type: application/json" \
#   -d '{"model":"bge-m3","input":["테스트"]}' | jq '.embeddings | length'
# Manual: 응답이 200 OK + embeddings 길이 ≥ 1 이어야 진행 의미 있음

# Manual: secall 통합 확인
# 1) config 수정: embedding.backend = "ollama_cloud"
# 2) cargo run -p secall -- ingest <단일 세션 경로>
# Manual: stderr 에 "Ollama Cloud embedder ready" 로그 + 정상 embedding 완료
```

## Risks

- **Ollama Cloud embedding 미지원 가능성** — Cloud 카탈로그가 chat 중심이라 bge-m3 같은 임베딩 모델이 없을 수 있음. Manual smoke 에서 확인 못하면 본 task 는 "dispatch 추가 + 안내 에러" 로 종결.
- **API key 노출 위험** — env 만 사용 권장. REST PATCH 차단 + GET 마스킹 적용 필수.
- **`OllamaEmbedder.api_key` 직렬화** — `Debug`/`Display` impl 에 key 가 노출되지 않도록 주의. `#[serde(skip_serializing)]` 또는 별도 redact.
- **로컬 폴백 시 사용자 혼란** — `embedding.backend = "ollama_cloud"` 인데 키가 없어 로컬로 폴백하면 stderr 경고만으로 부족할 수 있음. 명확한 한 줄 안내 ("set OLLAMA_CLOUD_API_KEY env to enable cloud embedding").
- code-review-graph 영향: `OllamaEmbedder` 시그니처 변경은 호환 (필드 추가 + builder). `create_vector_indexer` 의 새 arm 추가는 회귀 위험 낮음.

## Scope boundary (수정 금지)

- `OrtEmbedder` / `OpenVinoEmbedder` / `OpenAIEmbedder` — Task 01, 02 영역 또는 별개.
- `OrtEmbedder::with_pool_size` 호출부 — Task 02 영역.
- `ingest.rs` 의 embed unload — Task 04 영역.
- `docs/reference/llm-config.md`, `SettingsRoute.tsx` — Task 05 영역.
- 외부 ingest 코드 (`crates/secall-core/src/ingest/*`) — 본 plan 영역 밖.
