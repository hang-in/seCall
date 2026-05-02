---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p37-graph-sync
task_id: 02
parallel_group: B
depends_on: [01]
---

# Task 02 — REST `/api/commands/graph-rebuild` + Job 어댑터 + P36 cancel 지원

## Changed files

수정:
- `crates/secall-core/src/jobs/types.rs:9-15` — `JobKind` enum 에 `GraphRebuild` variant 추가, `as_str` / `from_str` 매핑에 `"graph_rebuild"` 추가.
- `crates/secall-core/src/jobs/adapters/mod.rs:57-66` — `CommandAdapters` 구조체에 `graph_rebuild_fn: GraphRebuildAdapterFn` 필드 추가, 본문에 `pub use graph_rebuild_adapter::GraphRebuildAdapterFn;` + `pub type GraphRebuildArgsValue = serde_json::Value;` 추가.
- `crates/secall-core/src/jobs/adapters/mod.rs` — module 선언에 `pub mod graph_rebuild_adapter;` 추가.
- `crates/secall-core/src/mcp/rest.rs:150-152` — 라우터에 `.route("/api/commands/graph-rebuild", post(api_command_graph_rebuild))` 추가.
- `crates/secall-core/src/mcp/rest.rs:461` 인접 — `JobKind::GraphRebuild => (adapters.graph_rebuild_fn)(args_value, sink)` 분기 추가.
- `crates/secall-core/src/mcp/rest.rs:495-512` 인접 — `api_command_sync` / `api_command_ingest` 패턴 그대로 따라 `api_command_graph_rebuild` 핸들러 추가. body 는 `GraphRebuildArgs` (Task 01 정의) 직렬화 형태.
- `crates/secall-core/src/mcp/rest.rs:184` (또는 endpoint list 로그) — `/api/commands/graph-rebuild` 추가 명시.
- `crates/secall/src/commands/serve.rs` (또는 `main.rs` 의 with_adapters 호출처) — `CommandAdapters` 인스턴스화 시 `graph_rebuild_fn: Box::new(...)` 주입. 호출 본문은 `secall::commands::graph::run_rebuild(args, sink).await` 위임.

신규:
- `crates/secall-core/src/jobs/adapters/graph_rebuild_adapter.rs` — `IngestAdapterFn` / `WikiUpdateAdapterFn` 패턴 그대로:
  - `pub type GraphRebuildAdapterFn = AdapterFn;`
  - 모듈 doc 주석으로 호출 계약 + `crates/secall/src/commands/graph.rs` 가 실제 클로저를 만든다는 indirection 설명.

## Change description

### 1. `JobKind::GraphRebuild` 추가 (types.rs)

기존 enum 에 variant 추가, snake_case 직렬화 → `"graph_rebuild"` (REST status, DB jobs.kind 컬럼 호환). `as_str` / `from_str` 매핑 동기화.

### 2. `CommandAdapters` 확장 (adapters/mod.rs)

```rust
pub struct CommandAdapters {
    pub sync_fn: SyncAdapterFn,
    pub ingest_fn: IngestAdapterFn,
    pub wiki_update_fn: WikiUpdateAdapterFn,
    pub graph_rebuild_fn: GraphRebuildAdapterFn, // 신규
}
```

신규 `graph_rebuild_adapter.rs` 는 P33 의 다른 어댑터와 같은 형태 (type alias + doc only, 실제 클로저는 secall 측에서 인스턴스화).

### 3. REST 라우터 + 핸들러 (rest.rs)

- 라우터에 `/api/commands/graph-rebuild` 추가
- spawn 분기에 `JobKind::GraphRebuild` 매핑 추가
- 핸들러 `api_command_graph_rebuild`: P33 의 `api_command_sync` 그대로 — `Json<GraphRebuildArgs>` 받아 `executor.try_spawn(JobKind::GraphRebuild, Some(args_json), |tx, token| ...)` 호출. 단일 큐 정책상 다른 mutating job 이 실행 중이면 409.

### 4. 어댑터 클로저 주입 (serve.rs / main.rs)

기존 sync/ingest/wiki 어댑터 클로저 옆에 graph_rebuild 클로저 추가. 본문은 args 를 `GraphRebuildArgs` 로 deserialize → `secall::commands::graph::run_rebuild(args, &BroadcastSink::new(tx, cancel_token)).await` → 결과를 `serde_json::to_value(outcome)` 로 직렬화 후 반환. P33 의 sync 어댑터 클로저 패턴 그대로.

### 5. P36 cancel 통합

별도 작업 없음 — Task 01 의 `run_rebuild` 가 sink 를 받아 `is_cancelled()` 폴링하고 partial outcome 반환. P36 `executor::try_spawn` 의 `was_cancelled` 게이팅이 status 강제 + partial_result 보존을 자동 처리.

### 6. 통합 테스트 (tests/jobs_rest.rs 또는 신규 tests/graph_rebuild_rest.rs)

기존 `tests/jobs_rest.rs` 의 sync/ingest 통합 테스트 패턴 따라 1건 추가:
- `test_graph_rebuild_endpoint_starts_job` — REST POST → 200 + job_id 반환, GET /api/jobs/{id} 로 status 추적 가능 검증
- (가능하다면) cancel 통합 테스트도 1건 — POST /api/jobs/{id}/cancel → status=interrupted

## Dependencies

- 외부 crate: 없음
- 내부 task: **Task 01 완료 필수** — `secall::commands::graph::{run_rebuild, GraphRebuildArgs}` 가 어댑터 클로저에서 호출됨

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --test jobs_rest test_graph_rebuild
cargo test --all

# 라이브 (서버 + vault 필요):
# secall serve --bind 127.0.0.1:8080 &
# curl -X POST http://127.0.0.1:8080/api/commands/graph-rebuild -d '{"retry_failed":true}'
# → 200 { "job_id": "...", "status": "started" }
# curl http://127.0.0.1:8080/api/jobs/<id>
# curl -X POST http://127.0.0.1:8080/api/jobs/<id>/cancel  # P36 통합 검증
```

## Risks

- **JobKind enum 추가의 호환성**: snake_case `"graph_rebuild"` 를 DB jobs.kind 에 저장. 기존 row 영향 없음 (enum 확장은 후방 호환). `JobKind::from_str` 미매칭 케이스 처리 확인.
- **단일 큐 정책 (P33)**: graph_rebuild 가 실행 중이면 sync/ingest/wiki 도 차단. 사용자가 의도. 문서 (Task 05 README) 에 명시.
- **adapter indirection**: secall-core 가 secall 을 reverse 의존하지 않도록 `Box<dyn Fn>` 패턴 유지. 신규 graph_rebuild_adapter.rs 는 type alias + doc only.
- **REST 응답 시점**: `try_spawn` 즉시 반환 (job 등록만), 실제 실행은 background. 클라이언트는 SSE 또는 polling 으로 추적 — 기존 패턴 그대로.
- **cancel 시 partial_outcome 직렬화**: `GraphRebuildOutcome` 가 serde::Serialize derive 되어 있어야 P36 executor 의 `result_json.clone()` 보존 경로가 동작.

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/`, `crates/secall-core/src/graph/` — Task 00 / 무관
- `crates/secall/src/commands/{graph,ingest,sync,wiki,mod}.rs` 의 본문 — Task 01 영역. 단 `serve.rs` (또는 main.rs) 의 with_adapters 주입 한 줄 추가는 본 task.
- `web/` — Task 03 영역
- `README*`, `.github/` — Task 04 영역
- `crates/secall-core/src/jobs/{registry,executor,mod}.rs` — P33/P36 완료 코드, 본 task 는 추가 없음
- 기존 `ProgressEvent` variant — 추가/수정 없음
