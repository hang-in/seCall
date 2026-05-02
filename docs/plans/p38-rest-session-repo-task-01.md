---
type: task
status: draft
updated_at: 2026-05-03
plan_slug: p38-rest-session-repo
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — REST 라우트 회귀 — 데이터 read 계열

## Changed files

수정:
- `crates/secall-core/tests/rest_routes.rs` — Task 00 의 sanity 테스트 옆에 read 라우트 11 종 회귀 테스트 추가 (Section 1: read routes). `mod common;` 으로 Task 01 fixture 사용.

신규: 없음 (파일은 Task 00 이 만듦)

## Change description

### 대상 라우트 (11 종)

| Method | Path | 핸들러 | 테스트 시나리오 |
|---|---|---|---|
| POST | `/api/recall` | `api_recall` | RestRecallParams DTO 변환 (mode 분기 keyword/semantic/temporal) + happy path 200 + 빈 query 처리 |
| POST | `/api/get` | `api_get` | session_id 존재/미존재 200/error, full=true 시 content 키 포함 |
| GET | `/api/status` | `api_status` | 200 + JSON object (Task 01 sanity 와 분리하여 응답 키 검증) |
| POST | `/api/wiki` | `api_wiki` | WikiSearchParams happy path 200 |
| GET | `/api/wiki/{project}` | `api_wiki_get` | 미존재 project 404 + msg, 존재 시 200 + content 필드 |
| POST | `/api/graph` | `api_graph` | RestGraphParams DTO 변환 + 200 |
| POST | `/api/daily` | `api_daily` | date 미지정 (오늘) + 명시 ("2026-04-01") 양쪽 200 |
| GET | `/api/sessions` | `api_list_sessions` | filter (project/agent/tag/tags/favorite/since/page) 종합 happy path. 기존 `rest_listing.rs` 회귀와 의도적 일부 중복 (라우트 레벨 검증 추가) |
| GET | `/api/projects` | `api_list_projects` | 빈 DB 빈 배열, 세션 추가 후 distinct project 반환 |
| GET | `/api/agents` | `api_list_agents` | 빈 DB 빈 배열, 세션 추가 후 distinct agent 반환 |
| GET | `/api/tags?with_counts={bool}` | `api_list_tags` | with_counts true 기본 → `[{name, count}]`, false → `[name]` 분기 검증 |

### 공통 패턴 (각 테스트)

1. `let env = make_test_env().await;`
2. (필요 시) `env.insert_minimal_session(...)` + `env.db.lock().unwrap().update_session_*(...)` 로 fixture 데이터
3. `let (status, body) = send_request(&env.router, Method::POST, "/api/recall", Some(json!({...}))).await;`
4. `assert_eq!(status, StatusCode::OK)` + `assert_eq!(body["count"], expected)` 등 응답 검증
5. error 분기: 잘못된 입력 → 400/500 + `body["error"]` 존재 검증

### DTO 변환 회귀 (Section 1A 별도 묶음)

`RestRecallParams` / `RestGetParams` / `RestGraphParams` 의 `From` impl 은 라우트 통과 시 자동 행사되지만, mode 분기처럼 미묘한 부분은 별도 mini 테스트로 명시:
- `test_recall_dto_mode_keyword_default` — mode 미지정 → QueryType::Keyword
- `test_recall_dto_mode_semantic` — `"semantic"` → QueryType::Semantic
- `test_recall_dto_mode_temporal` — `"temporal"` → QueryType::Temporal
- `test_get_dto_session_id_renamed` — `session_id` 필드가 `id` 로 매핑

이 4개는 `From` impl 직접 호출 (라우트 우회) — DTO 변환 자체 회귀.

### 응답 형태 검증 깊이

- happy path: status + 응답 JSON 의 핵심 키 1~3 개 (전체 schema 검증 X — 향후 변경 fragility 방지)
- error path: status (400/404/500) + `body["error"]` 문자열 존재
- 빈 결과: `count == 0`, items 빈 배열 등

### 라우트별 fixture 강도

- DB 가 비어 있어도 통과: status, projects, agents, tags
- session 1~3 개 필요: get, sessions, recall, daily
- vault 파일 필요: wiki/{project}, get(full=true) — temp wiki dir 만들어 빈 파일 생성

## Dependencies

- 외부 crate: 없음 (Task 00 의 dev-dep 사용)
- 내부 task: **Task 00 완료 필수** — `tests/common/mod.rs` 의 `TestEnv`, `send_request`, `make_fake_adapters` API 가 있어야 함

## Verification

```bash
cargo check --tests
cargo clippy --tests --all-features -- -D warnings
cargo fmt --all -- --check
cargo test -p secall-core --test rest_routes
```

15+ 테스트 (11 라우트 happy + 4 DTO + 일부 error 분기) 통과 목표.

## Risks

- **vault 파일 의존**: wiki/get(full=true) 가 vault 파일 읽음. tempdir 안에 stub markdown 파일 만들거나 분기로 우회.
- **embedding 의존 (recall semantic)**: vector backend 없는 환경에서는 빈 결과 반환 (P34 graceful) → semantic 분기 테스트는 빈 결과 검증만.
- **graph 데이터 의존**: `/api/graph` 가 빈 그래프에서도 동작 (results: []) → 빈 결과 검증만.
- **session_repo 의존**: list_sessions / projects / agents 는 P32~P37 에 따라 응답 형태 변경됨. 현재 contract 기준 작성 → 후속 변경 시 본 테스트가 회귀 검출 (의도).
- **응답 schema fragility**: 너무 strict 하게 모든 키 검증하면 향후 변경 시 테스트 깨짐. 핵심 키 (count, items, results, error) 만 검증.

## Scope boundary

수정 금지:
- `crates/secall-core/src/` 전체 production 코드
- `crates/secall-core/tests/common/mod.rs` — Task 00 영역 (단, 추가 fixture 필요 시 본 task 에서 helper 함수 1~2 개 추가는 OK)
- `crates/secall-core/tests/{rest_listing,jobs_rest,graph_incremental}.rs` — 무관
- 본 phase 의 다른 task 영역 (Task 02 의 write/commands/jobs 라우트, Task 03 의 session_repo helpers)
- `crates/secall/`, `web/`, `README*`, `.github/`
