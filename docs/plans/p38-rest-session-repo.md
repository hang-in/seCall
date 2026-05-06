---
type: plan
status: draft
updated_at: 2026-05-03
slug: p38-rest-session-repo
version: 1
---

# P38 — 테스트 갭 메우기 (REST 라우트 + session_repo)

## Description

P32~P37 동안 짧은 기간에 대량 신규 코드 (REST endpoint 22+, Job 시스템, P36 cancellation, P37 graph sync) 가 들어갔다. 회귀 안전망은 기존 `tests/rest_listing.rs`(12), `tests/jobs_rest.rs`(8), `tests/graph_incremental.rs`(4) + 모듈 inline 테스트 일부에 그치고, **REST 라우트 자체** (axum Router 에 대해 실제 Request/Response 통합 검증) 는 부재. P37 reviewer 도 동일 지적: "executor 직접 호출 수준 — route-level 테스트 별도".

본 phase 는 안전망 확장에 집중. 신규 기능 추가 없음.

## 현재 한계

- `crates/secall-core/src/mcp/rest.rs:122-170` — Router 정의 + 22 핸들러 모두 axum 통합 테스트 부재.
- `crates/secall-core/src/mcp/rest.rs` 의 RestRecallParams / RestGetParams / RestGraphParams 등 DTO 변환에 회귀 없음.
- `crates/secall-core/src/store/session_repo.rs:236-1075` — P34/P35/P37 신규 helper (`get_session_stats`, `list_all_tags`, `update_semantic_extracted_at`, `list_sessions_for_graph_rebuild`, `update_session_notes`, `update_session_favorite` 등) 일부만 inline 테스트, P37 helper 는 `tests/rest_listing.rs` 한 곳에서만 검증.
- Insight findings 다수 미해결: `TES-classify-rs`, `TES-graph-rs (×2)`, `TES-log-rs`, `TES-session_repo-rs-722줄`, `TES-session_repo-rs-trait`, `TES-sessionrepo-trait`, `TES-db-메서드-get_sessions_for_date`, `TES-graph-build-시-파일-읽기-파싱-실패-continue`.

## Expected Outcome

- `crates/secall-core/tests/rest_routes.rs` 신규 — axum `Router` + `tower::ServiceExt::oneshot` 기반 통합 테스트 인프라 + 핵심 라우트 회귀 (22 핸들러 happy path + 주요 error 분기).
- `crates/secall-core/tests/session_repo_helpers.rs` 신규 — P32~P37 누적 helper 한 자리 회귀 (favorite / notes / tags / multi-tag / get_session_stats / list_all_tags / semantic_extracted_at / list_for_graph_rebuild).
- README/CI 변경 없음 (기존 cargo test job 이 신규 테스트 자동 실행).
- Insight TES findings 일부 (REST + session_repo 영역) 가 본 phase 로 해소 → `docs/insight/findings/` 에 status 갱신.

## Subtasks

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 00 | axum Router 통합 테스트 인프라 (test helper + dev-dep) | A | — |
| 01 | REST 라우트 회귀 — 데이터 read 계열 (recall/get/status/wiki/graph/daily/sessions/projects/agents/tags) | B | 00 |
| 02 | REST 라우트 회귀 — write/commands/jobs (PATCH tags\|favorite\|notes + commands/sync\|ingest\|wiki-update\|graph-rebuild + jobs/list\|get\|stream\|cancel) | B | 00 |
| 03 | `session_repo` helper 회귀 통합 — P32~P37 신규 메서드 한 자리에 모음 | C | — |
| 04 | README 회귀 안전망 안내 + Insight findings 해결 표시 | D | 00, 01, 02, 03 |

병렬 실행 전략:
- Phase A — Task 00 (test infrastructure 단독)
- Phase B — Task 01 + 02 동시 dispatch (다른 라우트군, 인프라 의존만 같음)
- Phase C — Task 03 (인프라 무관, Phase A/B 와 병렬 가능 — 단 task 03 은 별도 dispatch)
- Phase D — Task 04 (모든 task 완료 후 README + Insight)

## Constraints

- **수정 금지**: P32~P37 production 코드 동작 변경. 본 phase 는 테스트만 (추가, 또는 Insight finding 의 status 메타 갱신).
- 기존 inline 테스트 / `tests/*` 파일 보존. 신규 라우트 테스트는 라우트 레벨, 기존 inline 은 모듈 단위 → 의도적 중복 일부 허용.
- axum 통합 테스트는 in-process Router 사용 (HTTP listen 안 함 — `tower::ServiceExt::oneshot`) → 빠름 + flake 없음.
- DB 는 `tempfile::tempdir() + Database::open(path)` 패턴 (`SECALL_DB_PATH` env 의존 회피 — P37 task 01 ENV_LOCK 같은 직렬화 문제 안 만듦).
- Job adapter 는 sync 처리 가능한 fake (P36/P37 jobs_rest 패턴 그대로) — 외부 의존성 없음.

## Non-goals

- LLM 백엔드 모킹 (Gemini/Haiku/Claude) — wiki/graph rebuild 의 실 LLM 호출은 별도 phase. 본 phase 의 wiki/graph 라우트 테스트는 NoopBackend 또는 빈 결과 분기만.
- Vector embedding (Ollama/OpenVINO) 단위 테스트 — 외부 서비스 의존, 별도 phase.
- web E2E (Playwright) — 별도 phase.
- 미테스트 CLI 명령 모듈 (`commands/{init,migrate,embed,recall,reindex,model,classify,log,graph}.rs`) — 본 phase 는 REST + repo 에 집중. CLI 단위 테스트는 P39 후보.
- coverage 측정 도구 (tarpaulin/llvm-cov) — 도구 도입은 별도 phase.
- SSE 스트림 종료 / lagged 시나리오 — `/api/jobs/{id}/stream` 은 happy path 만, 복잡한 lagged/disconnect 는 별도.

## Success criteria

- `cargo test --test rest_routes` — 25+ tests 통과 (라우트별 happy path + 주요 error 분기).
- `cargo test --test session_repo_helpers` — 20+ tests 통과 (P32~P37 helper 모두 행사).
- `cargo test --all` 전체 통과, 회귀 0.
- 기존 `tests/rest_listing.rs` / `tests/jobs_rest.rs` / `tests/graph_incremental.rs` 24 tests 그대로 유지.
- 신규 70+ tests 추가 → 후속 PR 의 라우트 / DTO / repo 변경 회귀 즉시 검출.
