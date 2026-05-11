---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P47
task_id: 04
parallel_group: 3
depends_on: [03]
---

# P47 Task 04 — ingest 종료 후 Ollama embed unload

## Changed files

### Ingest 종료 시점 unload

- `crates/secall/src/commands/ingest.rs:586-627` — `if !no_embed && !vector_tasks.is_empty()` 블록의 embed sub-loop 종료 직후 (= `vector_tasks` 순회 끝난 직후, `if semantic_enabled` 진입 전) 에 다음 추가:
  ```text
  // P47 — embed 단계 끝나면 Ollama 의 bge-m3 모델을 즉시 unload
  // (M4 Air 16GB 환경에서 keep_alive 만료 대기 동안 메모리 점유로 인한 swap 회피)
  if !no_embed && !vector_tasks.is_empty() {
      unload_ollama_embed_model(config).await;
  }
  ```
- `crates/secall/src/commands/ingest.rs:778-797` — 기존 `unload_embedding_model_if_needed` 는 "graph semantic 진입 직전" 용도(ollama backend + ollama graph backend 둘 다 일 때만 발사) 라서 본 task 요구와 다름. 신규 헬퍼 `unload_ollama_embed_model(config)` 를 추가:
  ```text
  /// P47 — embed 단계 종료 후 Ollama embedding 모델 즉시 unload.
  /// graph semantic 단계 진입 여부와 무관하게, ollama 백엔드 사용 시 항상 호출.
  pub async fn unload_ollama_embed_model(config: &Config) {
      if config.embedding.backend != "ollama" {
          return; // cloud / ort / openvino / openai 는 keep_alive 개념 없음
      }
      let embed_model = config.embedding.ollama_model.as_deref().unwrap_or("bge-m3");
      let ollama_url = config.embedding.ollama_url.as_deref().unwrap_or("http://localhost:11434");
      let unload_url = format!("{}/api/generate", ollama_url.trim_end_matches('/'));
      let body = serde_json::json!({"model": embed_model, "keep_alive": 0});
      if let Err(e) = secall_core::http_post_json(&unload_url, &body).await {
          tracing::debug!(model = embed_model, error = %e, "embed model unload skipped");
      } else {
          tracing::debug!(model = embed_model, "unloaded embedding model after ingest");
      }
  }
  ```
- 기존 `unload_embedding_model_if_needed` 는 graph semantic 진입 직전 호출용으로 그대로 유지 (`ingest.rs:637`, `graph.rs:253`). 두 헬퍼의 의도가 다르므로 별도 유지.

### 회귀 테스트

- `crates/secall/tests/` 신규 또는 기존 ingest 테스트에 — `unload_ollama_embed_model` 이:
  - `embedding.backend == "ort"` / `"ollama_cloud"` / `"openai"` 일 때 HTTP 호출 발사하지 않는지 (mockito 또는 단순 unit test for early return),
  - `embedding.backend == "ollama"` 일 때 `keep_alive: 0` body 로 POST 발사하는지 (mockito).
- 신규 파일 예: `crates/secall/tests/embed_unload.rs` (mockito 기반 단위 통합 테스트).

### Manual smoke

- ingest 종료 후 Activity Monitor 에서 Ollama 프로세스 메모리가 즉시 떨어지는지 (수십 MB 수준) 확인.

## Change description

### 단계별 접근

1. **헬퍼 추가** — `ingest.rs` 안에 `unload_ollama_embed_model` 신규 함수 추가. 기존 `unload_embedding_model_if_needed` 와 분리 (의도가 다름: 전자 = 종료 후 항상, 후자 = graph semantic 진입 직전 OOM 회피용).

2. **호출 지점 추가** — `vector_tasks` 순회 끝난 직후 (`ingest.rs:627` 부근). `!no_embed && !vector_tasks.is_empty()` 조건 일치할 때만 발사.

3. **다른 backend 빠르게 early return** — ort / openvino / openai / ollama_cloud 는 unload 의미 없음 (cloud 는 서버 측 lifecycle, ort/openvino 는 호출 종료 시 자동 unload).

4. **단위 테스트** — backend 별 분기 동작 검증.

### 구현 제약

- **기존 `unload_embedding_model_if_needed` 변경 X** — graph rebuild path 도 같은 헬퍼를 호출하므로 시그니처 보존.
- **에러는 tracing::debug 로만** — unload 실패해도 ingest 자체는 성공 처리. 사용자 메시지 없음 (verbose 한 경고는 노이즈).
- **`ollama_cloud` 는 unload 대상 아님** — Cloud 측 model lifecycle 은 우리가 제어 못함. early return.

## Dependencies

- **Task 03 완료 후** — `embedding.backend` 가 `"ollama_cloud"` 값을 가질 수 있어야 early return 분기 의미 있음.

## Verification

```bash
# 타입체크
cargo check -p secall

# 단위 테스트
cargo test -p secall --lib commands::ingest::tests
cargo test -p secall --test embed_unload  # 신규 테스트 파일 추가 시

# Manual: 16GB 시스템에서 ingest 종료 후 메모리 회복 확인
# Activity Monitor 또는 vm_stat 으로 Ollama 프로세스 메모리 측정
# cargo run -p secall -- ingest <single session>
# Manual: ingest 완료 직후 Ollama 메모리가 즉시 회복 (≤ 100MB) 되어야 함
# Manual: 비교 — Task 04 적용 전에는 keep_alive 만료까지 ~5분 대기
```

## Risks

- **`/api/generate` 가 embedding 모델에도 동작하는지** — 기존 `unload_embedding_model_if_needed` 가 동일 패턴(`/api/generate` + `keep_alive: 0`) 사용하는데 P37 이후 production 에서 검증됐으므로 동작 확인됨. 만약 embedding-only 모델에선 `/api/embed` + `keep_alive: 0` 사용 필요할 수 있음 — Task 04 manual smoke 에서 둘 다 시도.
- **Unload race condition** — embed sub-loop 끝나자마자 semantic 단계가 다시 모델 로드 (다른 모델). 동시 unload+load 가 일어나도 Ollama 가 serialize 처리하므로 문제 없을 것. 회귀 발견 시 graph semantic 진입 전에 짧은 sleep 추가.
- **Cloud / ort 백엔드에서 무의미 호출** — early return 으로 차단했지만, backend 문자열이 예상 외 값일 때 (`"none"`) 도 early return 되도록 명시.
- code-review-graph 영향: `ingest.rs::run_with_progress` 에 호출 한 줄 추가. 기존 sub-loop 종료 후 흐름은 그대로.

## Scope boundary (수정 금지)

- `crates/secall/src/commands/ingest.rs:778-797` 의 기존 `unload_embedding_model_if_needed` — graph 호출자도 공유하므로 시그니처/동작 보존.
- `crates/secall/src/commands/graph.rs` — graph rebuild path 는 별개.
- `crates/secall-core/src/search/embedding.rs` — Task 01/03 영역.
- `crates/secall-core/src/search/vector.rs` — Task 02/03 영역.
- `docs/`, `web/` — Task 05 영역.
