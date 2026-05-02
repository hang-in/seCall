---
type: task
status: draft
updated_at: 2026-05-03
plan_slug: p38-rest-session-repo
task_id: 02
parallel_group: B
depends_on: [00]
---

# Task 02 — REST 라우트 회귀 — write / commands / jobs

## Changed files

수정:
- `crates/secall-core/tests/rest_routes.rs` — Task 01 의 read 라우트 옆에 write/commands/jobs 라우트 11 종 회귀 테스트 추가 (Section 2). 같은 `mod common;` 사용.

신규: 없음 (파일은 Task 00 이 만듦)

## Change description

### 대상 라우트 (11 종)

**Write (PATCH) — 3종**
| Method | Path | 핸들러 | 시나리오 |
|---|---|---|---|
| PATCH | `/api/sessions/{id}/tags` | `api_set_tags` | body `{tags: ["rust","db"]}` 200 + 정규화/dedup 결과, 미존재 id error |
| PATCH | `/api/sessions/{id}/favorite` | `api_set_favorite` | body `{favorite: true}` 토글, 미존재 id error |
| PATCH | `/api/sessions/{id}/notes` | `api_set_notes` | body `{notes: "..."}`, null 입력 (지우기), 미존재 id error |

**Commands (POST) — 4종**
| Method | Path | 핸들러 | 시나리오 |
|---|---|---|---|
| POST | `/api/commands/sync` | `api_command_sync` | body SyncArgs 200 + `{job_id, status: "started"}`, 동시 실행 중이면 409 |
| POST | `/api/commands/ingest` | `api_command_ingest` | body IngestArgs 200, dry_run 검증 |
| POST | `/api/commands/wiki-update` | `api_command_wiki_update` | body WikiUpdateArgs 200 |
| POST | `/api/commands/graph-rebuild` | `api_command_graph_rebuild` | body GraphRebuildArgs (P37) 200, retry_failed/all/since/session 분기 |

**Jobs — 4종**
| Method | Path | 핸들러 | 시나리오 |
|---|---|---|---|
| GET | `/api/jobs?status={active\|recent}` | `api_list_jobs` | 빈 목록, 진행 중 job 1개 등록 후 active 결과 |
| GET | `/api/jobs/{id}` | `api_get_job` | 진행 중 / 완료 / 미존재 (404) |
| GET | `/api/jobs/{id}/stream` | `api_job_stream` | SSE happy path — 첫 이벤트 (initial_state) 수신, 단발 검증으로 충분 (lagged/disconnect 는 본 phase 외) |
| POST | `/api/jobs/{id}/cancel` | `api_cancel_job` | 활성 job 200 idempotent, 이미 완료된 job 200 idempotent, 미존재 404 |

### 단일 큐 정책 회귀 (별도 묶음)

P33 단일 큐 정책: `/api/commands/*` 4 라우트가 동시 실행 시 두 번째는 409.
- `test_commands_single_queue_returns_409` — sync spawn 후 즉시 graph-rebuild → 409 + `{error}`

### Cancel 흐름 통합 (별도 묶음)

P36 cancel: spawn → cancel → SSE Failed event + status=interrupted.
- `test_command_then_cancel_yields_interrupted` — 충분히 긴 fake adapter spawn → cancel POST → GET /api/jobs/{id} status=interrupted 검증

`tests/jobs_rest.rs` 의 `test_graph_rebuild_cancel_interrupts_job` 와 패턴 동일하지만 본 task 는 라우트 레벨 (axum oneshot) 통과 검증.

### Body 직렬화/역직렬화

각 commands 엔드포인트의 body 는 P37 web UI 에서 사용하는 JSON 형태와 일치하는지 검증 — 이게 contract drift 회귀 안전망.

### 공통 패턴

```text
let env = make_test_env().await;
let (status, body) = send_request(&env.router, Method::POST, "/api/commands/sync",
    Some(json!({"local_only": true, "dry_run": true}))).await;
assert_eq!(status, StatusCode::OK);
assert!(body["job_id"].is_string());
assert_eq!(body["status"], "started");
```

### 회귀 강도

- happy path: status + 핵심 키 1~3 개
- error: status (400/404/409) + `body["error"]`
- cancel: status 전이 검증 (Started → Running → Interrupted) — 짧은 sleep 필요

## Dependencies

- 외부 crate: 없음
- 내부 task: **Task 00 완료 필수** — TestEnv 의 fake adapters 가 single queue / cancel 시나리오 행사 가능해야 함

## Verification

```bash
cargo check --tests
cargo clippy --tests --all-features -- -D warnings
cargo fmt --all -- --check
cargo test -p secall-core --test rest_routes
```

15+ 테스트 (write 3 × happy/error + commands 4 × happy + jobs 4 + 통합 시나리오 2~3) 통과 목표.

## Risks

- **단일 큐 race**: P33 spawn_gate 가 정확히 직렬화 → 첫 spawn 이 충분히 살아있도록 fake adapter 가 sleep. 너무 짧으면 두 번째 spawn 도 OK 받을 가능성.
- **SSE 테스트 복잡도**: `Sse<Stream>` 응답을 axum oneshot 으로 받으면 body 가 stream → 한 chunk 만 읽고 종료. body extract 패턴 검증 필요. 너무 복잡하면 `test_job_stream_single_event_smoke` 정도로 단순화.
- **fake adapter 결과 직렬화**: outcome JSON 이 deserialize 가능해야 GET /api/jobs/{id} 가 정상 응답. SyncOutcome / IngestOutcome / WikiOutcome / GraphRebuildOutcome 모두 fake 가 적절한 dummy 반환.
- **timing 의존**: cancel 테스트는 spawn → status running 대기 → cancel → status interrupted 대기. tokio sleep 100~500ms 필요. CI 느릴 때 flake 가능 → 적당한 timeout (1~2초).

## Scope boundary

수정 금지:
- `crates/secall-core/src/` 전체 production 코드
- `crates/secall-core/tests/common/mod.rs` — Task 00 영역 (helper 보강은 본 task 에서 OK)
- `crates/secall-core/tests/{rest_listing,jobs_rest,graph_incremental}.rs` — 무관
- Task 01 의 read 라우트 테스트 — 같은 파일이지만 Section 분리
- Task 03 의 session_repo helpers — 무관
- `crates/secall/`, `web/`, `README*`, `.github/`
