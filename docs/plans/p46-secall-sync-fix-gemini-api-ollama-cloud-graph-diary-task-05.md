---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P46
task_id: 05
parallel_group: 4
depends_on: [04]
---

# P46 Task 05 — 회귀 테스트 + 문서 + web UI

## Changed files

### 회귀 테스트 보강

- `crates/secall/tests/log_backend_resolve.rs` — 신규 케이스:
  - `ollama_cloud` backend 가 `[log].cloud_model` → `[graph].cloud_model` → `LOG_OLLAMA_CLOUD_DEFAULT` 순서로 해소되는지.
  - `OLLAMA_CLOUD_API_KEY` env 가 `[graph].cloud_api_key` 와 `[log].cloud_api_key` 양쪽에 반영되는지 (`ENV_MUTEX` 사용).
- `crates/secall-core/tests/llm_defaults.rs` — 신규 케이스:
  - `GRAPH_OLLAMA_CLOUD_DEFAULT == "gemma4:31b-cloud"` (Ollama Cloud 카탈로그 변경 detection 용 sentinel).
  - `LOG_OLLAMA_CLOUD_DEFAULT == "kimi-k2.6:cloud"`.
  - `LOG_CONTEXT_CHAR_LIMIT` 값이 양수이고 합리적 범위 (예: 200_000 이상 1_000_000 이하).
- `crates/secall-core/tests/rest_config.rs` — 신규 케이스:
  - `PATCH /api/config/graph` body 에 `cloud_api_key` 가 와도 server 가 무시 (write-only env 정책). 기존 `test_patch_graph_section_ignores_gemini_api_key` 와 동일 패턴.
  - `GET /api/config` 응답에 `cloud_api_key` 가 노출되지 않음 (마스킹).

### 문서 갱신

- `docs/reference/llm-config.md:18-23` — backends 표:
  - `gemini` 행 삭제.
  - `ollama_cloud` 행 추가 (`wiki | graph | log | review | embed` 컬럼 — graph/log 만 ✅).
- `docs/reference/llm-config.md:25-32` — 환경변수 표:
  - `SECALL_GEMINI_API_KEY` 행 삭제.
  - `OLLAMA_CLOUD_API_KEY` 행 추가 (`ollama_cloud` graph or log backend).
- `docs/reference/llm-config.md:36-46` — Config Keys 표:
  - `[graph].gemini_model` 행 삭제.
  - `[graph].cloud_host` (`https://ollama.com`), `[graph].cloud_model` (`gemma4:31b-cloud`), `[log].cloud_host`, `[log].cloud_model` (`kimi-k2.6:cloud`) 행 추가.
- `docs/reference/index.md:30-40` 부근 — `SECALL_GEMINI_API_KEY` / `SECALL_GRAPH_API_KEY` 우선순위 관련 문장 정리. `OLLAMA_CLOUD_API_KEY` 안내 추가.
- `docs/reference/daily-host-suffix-handoff.md:80` — `semantic_backend = "ollama"` 예시 컨텍스트 검토 (변경 불필요할 가능성 높음).
- `README.md:549` — `default_backend = "lmstudio"` 주석의 backend 리스트에서 별다른 변경 없음 (wiki 백엔드라 Gemini 미포함). 하지만 graph/log 섹션 설명이 있다면 거기 갱신.
- `README.en.md:571` — 동일.
- `docs/plans/p46-...-result.md` — **작성 금지** (tunaFlow 자동 생성).

### Web UI

- `web/src/routes/SettingsRoute.tsx:188` — `MaskedKeyInfoModal` 의 `envVar="SECALL_GEMINI_API_KEY"` 를 `envVar="OLLAMA_CLOUD_API_KEY"` 로 교체.
- `web/src/routes/SettingsRoute.tsx` — Settings 폼 안의 Graph / Log 섹션에서 Gemini 모델 입력 행 제거 + Ollama Cloud `cloud_host` / `cloud_model` 입력 행 추가. (정확한 라인은 Task 03 의 config schema 가 web side 에 어떻게 노출되는지에 따라 다름 — 같은 컴포넌트 패턴을 따를 것.)
- `web/src/routes/SettingsRoute.test.tsx` — Gemini 관련 테스트 케이스 제거 또는 ollama_cloud 로 교체.

### Config test command 출력 정리

- `crates/secall/src/commands/config.rs:156-` `print_llm_summary` — Graph / Log 섹션 출력에서:
  - `gemini_model` 출력 라인 제거 (Task 02 에서 함께 처리 가능).
  - `cloud_host` / `cloud_model` 출력 라인 추가.

## Change description

### 단계별 접근

1. **테스트 먼저** — 회귀 테스트를 작성한 다음 Task 02~04 의 결과가 통과하는지 확인. 만약 누락된 부분이 있으면 해당 task 로 돌아가서 보완.

2. **문서 갱신** — backends 표, 환경변수 표, Config Keys 표 세 곳을 동시에 갱신. README 한/영 동기화.

3. **Web UI** — Gemini 입력 제거 + Ollama Cloud 입력 추가. SettingsRoute.test.tsx 도 함께 업데이트.

4. **마이그레이션 안내 추가** — `docs/reference/llm-config.md` 의 Troubleshooting 또는 Migration 섹션에 다음 안내 추가:
   ```text
   ## P46 마이그레이션 (Gemini → Ollama Cloud)

   기존 사용자가 `[graph] semantic_backend = "gemini"` 또는 `[log] backend = "gemini"`
   를 쓰고 있었다면 다음과 같이 변경:

   - config.toml 의 [graph] 섹션에서 gemini_api_key, gemini_model 줄 제거
   - semantic_backend = "ollama_cloud" 로 변경
   - .env 에 OLLAMA_CLOUD_API_KEY=<key> 설정
   - graph 와 log 가 다른 모델을 쓰려면 [graph].cloud_model 과 [log].cloud_model 분리 설정
   ```

### 구현 제약

- **문서 표 정렬 유지** — markdown 표 컬럼 정렬이 깨지지 않도록 pipe `|` 정렬 유지.
- **Web UI 동작 검증** — `npm run dev` 띄우고 Settings 페이지에서 Ollama Cloud 행 입력 → save → 다시 로드 시 값이 보이는지 manual 확인.
- **테스트 ENV 정리** — env 사용 테스트는 모두 `ENV_MUTEX` 또는 동등한 직렬화 mechanism 안에서 실행.

## Dependencies

- **Task 04 완료 필수** — defaults 와 가드까지 들어간 상태에서 문서/테스트 정리.

## Verification

```bash
# 타입체크
cargo check -p secall-core
cargo check -p secall

# 회귀 테스트
cargo test -p secall --test log_backend_resolve
cargo test -p secall-core --test llm_defaults
cargo test -p secall-core --test rest_config

# 전체 lib + 통합 테스트 (정합성 확인)
cargo test -p secall-core --lib
cargo test -p secall --lib

# Web
cd web && npm run typecheck && npm test

# Manual: Settings UI 확인
# cd web && npm run dev
# Manual: http://localhost:5173/settings 에서:
#   - Gemini 관련 입력이 보이지 않아야 함
#   - Ollama Cloud API key + cloud_host + cloud_model 입력이 보여야 함
#   - 마스킹된 키 모달 안내에 OLLAMA_CLOUD_API_KEY 가 표시되어야 함

# Manual: 문서 시각 검증
cat docs/reference/llm-config.md | head -50
# Manual: backends 표에 gemini 가 없고 ollama_cloud 가 있어야 함
# Manual: 환경변수 표에 SECALL_GEMINI_API_KEY 가 없고 OLLAMA_CLOUD_API_KEY 가 있어야 함
```

## Risks

- **문서 누락 파일** — `docs/plans/` 의 과거 P26/P28/P30/P41/P43 plan 문서에 Gemini 언급이 다수 존재 (`grep -rn 'gemini' docs/`). **이건 historical record 이므로 수정하지 않음**. 새로 작성되는 문서만 ollama_cloud 기준.
- **Web SettingsRoute.tsx 구조 변경 파급** — Settings 폼이 backend 별로 동적 렌더링하는 구조라면 backend type 추가 후 ollama_cloud 케이스만 분기 추가하면 OK. 만약 backend 별 별도 컴포넌트라면 신규 컴포넌트 작성 필요.
- **사용자가 .env 의 Gemini 키를 그대로 둠** — `SECALL_GEMINI_API_KEY` 가 남아있어도 동작에는 영향 없음 (참조 코드 모두 제거). Task 05 의 마이그레이션 안내에 "키 삭제는 선택" 명시.
- **REST API breaking change** — `PATCH /api/config/graph` 의 `gemini_api_key` / `gemini_model` 필드를 수용하던 코드가 없어짐. 외부 client (Obsidian 플러그인 등) 가 이 필드를 보내면 단순 무시되어야 하지 (`#[serde(deny_unknown_fields)]` 아니면 OK), error 가 나면 안 됨. 확인 필요.
- code-review-graph 영향: 문서 + 테스트 변경은 함수 그래프에 영향 없음. Web UI 변경은 별도 그래프 (TypeScript 영역).

## Scope boundary (수정 금지)

- `crates/secall-core/src/ingest/gemini.rs`, `gemini_web.rs`, `detect.rs::find_gemini_sessions`.
- `crates/secall/src/commands/sync.rs` — Task 01 영역.
- Task 02~04 에서 이미 수정 완료한 코드 — 이 Task 에서는 정리/문서/테스트만 추가.
- 외부 historical plan 문서 (`docs/plans/p26-*`, `p28-*` 등) 의 Gemini 언급 — historical record 로 유지.
- 결과 보고서 `docs/plans/p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-result.md` — tunaFlow 자동 생성, 작성 금지.
