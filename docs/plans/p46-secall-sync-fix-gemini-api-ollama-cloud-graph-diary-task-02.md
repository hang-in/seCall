---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P46
task_id: 02
parallel_group: 1
depends_on: []
---

# P46 Task 02 — Gemini API 백엔드 호출 측 제거

## Changed files

### 코드 (Rust)

- `crates/secall-core/src/graph/semantic.rs:274` — `extract_with_gemini` 함수 전체 제거.
- `crates/secall-core/src/graph/semantic.rs:441` — `match config.semantic_backend.as_str() { ... "gemini" => extract_with_gemini(fm, body, config).await, ... }` 분기 제거.
- `crates/secall-core/src/graph/semantic.rs:818-825` — `test_extract_with_gemini_requires_api_key_before_network` 테스트 제거.
- `crates/secall/src/commands/log.rs:308-322` — `"gemini" =>` 분기 (lines 308~322) 제거.
- `crates/secall/src/commands/log.rs:327-` — `call_gemini` 함수 제거.
- `crates/secall/src/commands/log.rs:203-206` — `resolve_log_model` 의 `"gemini"` arm 제거.
- `crates/secall/src/commands/log.rs:4` — `use secall_core::llm::defaults::{..., LOG_GEMINI_DEFAULT, ...}` import 에서 `LOG_GEMINI_DEFAULT` 제거.
- `crates/secall/src/commands/log.rs:449` (테스트) — `config.graph.semantic_backend = "gemini"` 설정하는 테스트 케이스 정리 또는 갱신.
- `crates/secall/src/commands/config.rs:122-135` — `run_llm_test` 의 backends 리스트에서 `"gemini"` 제거 (`vec!["claude", ..., "gemini"]` → `"gemini"` 제외).
- `crates/secall/src/commands/config.rs:217` — `"<env: SECALL_GEMINI_API_KEY>"` 표시 제거.
- `crates/secall/src/commands/config.rs:292-293` — `SECALL_GEMINI_API_KEY` env 표시 제거.
- `crates/secall/src/commands/config.rs:453` — `test_gemini_backend` 함수 전체 제거.
- `crates/secall/src/commands/config.rs:458,461,466` — `test_gemini_backend` 내부의 `SECALL_GEMINI_API_KEY` 참조 정리.
- `crates/secall/src/commands/config.rs` — `test_gemini_backend` 호출하는 dispatch (보통 `match name { ... "gemini" => test_gemini_backend(...) }`) 도 함께 제거.
- `crates/secall-core/src/llm/defaults.rs:6` — `GRAPH_GEMINI_DEFAULT` 상수 제거.
- `crates/secall-core/src/llm/defaults.rs:12` — `LOG_GEMINI_DEFAULT` 상수 제거.
- `crates/secall-core/src/vault/config.rs:163` — `GraphConfig` 의 doc comment 에서 `"gemini"` 옵션 제거.
- `crates/secall-core/src/vault/config.rs:171-174` — `gemini_api_key: Option<String>`, `gemini_model: Option<String>` 필드 제거.
- `crates/secall-core/src/vault/config.rs:185-186` — `Default` impl 의 `gemini_api_key`, `gemini_model` 라인 제거.
- `crates/secall-core/src/vault/config.rs:372,377-379` — env override 의 `"gemini" => ...` arm + `SECALL_GRAPH_API_KEY` → `gemini_api_key` 매핑 제거. `SECALL_GRAPH_API_KEY` 자체는 ollama_cloud 용으로 Task 03 에서 재정의될 수 있음.
- `crates/secall-core/src/vault/config.rs:534` — `pub fn default_graph_gemini_model() -> &'static str` 함수 제거.
- `crates/secall-core/src/vault/config.rs:579,582,598-600` — 테스트 `test_graph_env_override_model_gemini` 제거 또는 다른 backend 로 교체.
- `crates/secall-core/src/main.rs:454` 부근 dotenv 로딩 주석 — `SECALL_GEMINI_API_KEY` 언급 제거.

### 테스트

- `crates/secall/tests/log_backend_resolve.rs:11` — `config.graph.semantic_backend = "gemini"` 케이스 제거 또는 다른 backend 로 교체.
- `crates/secall/tests/config_llm_test.rs:24` — `.env_remove("SECALL_GEMINI_API_KEY")` 라인 제거.
- `crates/secall-core/tests/rest_config.rs:177` — `test_patch_graph_section_ignores_gemini_api_key` 테스트 의도 검토. (Gemini 필드가 없어졌으므로 테스트가 의미를 잃음 → 제거 또는 ollama_cloud_api_key 로 의도 이전.)
- `crates/secall-core/tests/llm_defaults.rs` — `LOG_GEMINI_DEFAULT` / `GRAPH_GEMINI_DEFAULT` 참조 테스트 제거.

### Web

- `web/src/routes/SettingsRoute.tsx:188` — `envVar="SECALL_GEMINI_API_KEY"` 입력 row 제거. 이 입력의 부모 컴포넌트 (label / wrapper) 와 함께 깔끔히 제거.

### 문서 (참조만, 코드 task 종료 후 Task 05 에서 일괄)

이 Task 에서는 코드 제거에 집중. 문서는 Task 05 에서 일괄 갱신:
- `docs/reference/llm-config.md:30`
- `docs/reference/index.md:35`

## Change description

### 단계별 접근

1. **호출하는 쪽만 제거** — 외부 Gemini 세션 ingest 코드(`ingest/gemini.rs` 등)는 절대 건드리지 않음. Gemini **API key 기반 호출** 만 제거.

2. **dispatch arm 제거** — `graph/semantic.rs` 의 `match config.semantic_backend.as_str()` 에서 `"gemini"` arm 제거. `log.rs` 의 `match backend_name.as_str()` 에서도 동일.

3. **config schema 정리** — `GraphConfig` 의 `gemini_api_key` / `gemini_model` 필드 제거. 기존 `config.toml` 에 이 필드가 남아있어도 `#[serde(default)]` 덕분에 deserialize 는 실패하지 않지만, 알 수 없는 필드는 무시되는 동작에 의존하므로 Task 05 의 문서에 마이그레이션 안내 추가.

4. **env var 처리 정리** — `SECALL_GEMINI_API_KEY` 환경변수 참조 코드 제거. 사용자 .env 에 키가 남아있어도 동작에 영향 없게.

5. **테스트 정리** — Gemini 전용 테스트 제거. 다른 backend 로 의미 있게 변환 가능한 테스트는 ollama 로 교체.

6. **config test command 정리** — `secall config test` 의 backend 리스트에서 `gemini` 제거. 출력 포맷이 사용자 시각에 변경됨을 Task 05 의 문서에 반영.

### 구현 제약

- **단일 path 수정 원칙** — `graph/semantic.rs`, `log.rs`, `config.rs`, `vault/config.rs` 각 파일은 하나의 PR/커밋 안에서 함께 수정해야 컴파일 깨지지 않음 (cross-file 의존). 단, 파일별 변경은 독립적으로 검증 가능.
- **dispatch fallthrough 처리** — 기존 `"gemini" => ...` arm 을 제거할 때 `match` 의 `_ =>` arm (현재 `anyhow::bail!("unknown semantic_backend: {}", ...)`) 이 그대로 fallback 역할 → 별도 fallback 추가 X.
- **테스트 ENV_MUTEX 유지** — `vault/config.rs:543` 의 `ENV_MUTEX` 는 다른 env 테스트들도 사용 중. 제거하지 말 것.

## Dependencies

없음. Task 01 과 영역 분리되어 병렬 가능 (parallel_group 1).
**Task 03 은 이 Task 결과 위에서 ollama_cloud 식별자를 추가**하므로 Task 02 완료 후 시작.

## Verification

```bash
# 타입체크 — Gemini 참조 누락이 없는지
cargo check -p secall-core
cargo check -p secall

# 단위 테스트 — backend resolve / config schema / dispatch
cargo test -p secall-core --lib graph::semantic
cargo test -p secall-core --lib vault::config
cargo test -p secall --lib commands::log
cargo test -p secall --lib commands::config

# 통합 테스트 — backend resolve 통합 케이스
cargo test -p secall --test log_backend_resolve
cargo test -p secall --test config_llm_test
cargo test -p secall-core --test rest_config
cargo test -p secall-core --test llm_defaults

# grep 으로 잔존 참조 확인 (0 줄이어야 함)
grep -rn "extract_with_gemini\|call_gemini\|LOG_GEMINI_DEFAULT\|GRAPH_GEMINI_DEFAULT\|gemini_api_key\|gemini_model\|SECALL_GEMINI_API_KEY\|default_graph_gemini_model" \
  crates/secall/src crates/secall-core/src \
  --include='*.rs' \
  | grep -v 'crates/secall-core/src/ingest/gemini\(_web\)\?\.rs' \
  | grep -v 'crates/secall-core/src/ingest/detect.rs'
# ↑ 출력이 비어야 함 (ingest/gemini*.rs 와 detect.rs 의 find_gemini_sessions 만 살아있어야 함)

# Web 빌드 (Gemini key input 제거 확인)
cd web && npm run typecheck
```

## Risks

- **`config.toml` 의 stale 필드** — 사용자가 `[graph] gemini_api_key = "..."` 같은 줄을 갖고 있어도 `#[serde(default)]` 동작 덕에 deserialize 자체는 실패하지 않지만, toml-edit 기반 `save()` 가 unknown field 를 어떻게 다루는지 확인 필요. Task 05 의 문서에 "기존 사용자는 `[graph]` 섹션에서 gemini_* 줄 수동 삭제 권장" 안내 추가.
- **`SECALL_GRAPH_API_KEY` env 의 의미 변경** — 현재 `vault/config.rs:378` 에서 이 env 를 `gemini_api_key` 로 매핑하고 있음. 이 env 는 P26 시점에 추가됐고 docs/reference/index.md:35 에 "신규" 로 표시되어 있음. Task 03 에서 `OLLAMA_CLOUD_API_KEY` 로 재할당될 가능성 있음 → Task 02 에서는 단순히 매핑만 제거.
- **테스트 case 손실** — `test_graph_env_override_model_gemini`, `test_patch_graph_section_ignores_gemini_api_key` 등이 사라지면 env override / REST PATCH 보호 로직의 회귀 커버리지가 줄어듦. 가능하면 ollama 또는 (Task 03 이후) ollama_cloud 케이스로 의도 이전.
- **Web Settings 사용자 데이터 손실** — `SECALL_GEMINI_API_KEY` 입력 row 가 사라지면 사용자가 web UI 에서 키를 보거나 수정할 수 없음. 사용자가 .env 직접 편집 가능하므로 영향 작음.
- code-review-graph 영향: `graph/semantic.rs:441` 의 dispatch 분기 제거는 `extract_and_store` 호출 경로 (sync, ingest, graph rebuild 등) 에 영향. 그러나 호출자 입장에서는 `config.semantic_backend` 값만 바뀌면 되므로 회귀 위험 낮음.

## Scope boundary (수정 금지)

- `crates/secall-core/src/ingest/gemini.rs` — Gemini CLI 세션 ingest, 절대 손대지 말 것.
- `crates/secall-core/src/ingest/gemini_web.rs` — Gemini Web 세션 ingest, 절대 손대지 말 것.
- `crates/secall-core/src/ingest/detect.rs::find_gemini_sessions` — 외부 Gemini 세션 발견 함수, 유지.
- `crates/secall/src/commands/sync.rs` — Task 01 영역.
- `crates/secall/src/commands/log.rs` 의 ollama / claude / codex / haiku / lmstudio backend dispatch — 유지 (Gemini arm 만 제거).
- `crates/secall-core/src/wiki/**` — wiki 는 Gemini 미사용, 이 task 와 무관.
- Ollama Cloud 식별자 추가는 Task 03 영역.
