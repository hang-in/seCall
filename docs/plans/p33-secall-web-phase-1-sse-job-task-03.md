---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 03
parallel_group: D
depends_on: [02]
---

# Task 03 — REST 엔드포인트 (Jobs)

## Changed files

수정:
- `crates/secall-core/src/mcp/rest.rs` — 신규 라우트 6개 + SSE 핸들러 + AppState에 `JobExecutor` 포함
- `crates/secall-core/src/mcp/server.rs` — `do_*` 메서드 4개 (start_job, get_job, list_active_jobs, cancel_job placeholder)
- `crates/secall/src/commands/serve.rs` — `JobExecutor` 인스턴스 생성 + `CommandAdapters` 등록 + `start_rest_server`에 전달
- `crates/secall-core/Cargo.toml` — `tokio-stream`, `axum`은 이미 있음 (axum 0.8 SSE 지원 확인)

신규: 없음 (모두 수정)

## Change description

### 1. 엔드포인트 사양

| Method | Path | Body / Query | Response |
|---|---|---|---|
| `POST` | `/api/commands/sync` | `{ local_only?, dry_run?, no_wiki?, no_semantic? }` | `{ job_id, status: "started" }` 또는 409 `{ error, current_kind }` |
| `POST` | `/api/commands/ingest` | `{ cwd?, force?, min_turns?, no_semantic? }` | 동일 |
| `POST` | `/api/commands/wiki-update` | `{ backend?, model?, since?, session?, dry_run?, review? }` | 동일 |
| `GET` | `/api/jobs` | query: `status` (optional, 기본 active만) | `{ jobs: [JobState] }` |
| `GET` | `/api/jobs/:id` | — | `JobState` 또는 404 |
| `GET` | `/api/jobs/:id/stream` | — | `text/event-stream` (SSE) |
| `POST` | `/api/jobs/:id/cancel` | — | 501 Not Implemented (Phase 1+ — phase 경계 cancellation은 v1.1) |

### 2. AppState 확장

```rust
pub struct AppState {
    pub server: Arc<SeCallMcpServer>,
    pub executor: Arc<JobExecutor>,
}
```

기존 `type AppState = Arc<SeCallMcpServer>` 변경. 모든 기존 핸들러 시그니처도 `State<AppState>` → state.server 접근으로 갱신. 또는 두 개의 nested state 구조 (axum 0.8 지원).

### 3. POST /api/commands/* 핸들러

```rust
#[derive(Debug, Deserialize)]
struct SyncCmdBody {
    #[serde(default)]
    local_only: bool,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    no_wiki: bool,
    #[serde(default)]
    no_semantic: bool,
}

async fn api_command_sync(
    State(s): State<AppState>,
    Json(body): Json<SyncCmdBody>,
) -> impl IntoResponse {
    let metadata = serde_json::to_value(&body).ok();
    let args = SyncArgs::from(body);
    let executor = s.executor.clone();
    match executor.try_spawn(JobKind::Sync, metadata, move |tx| async move {
        (executor.adapters.sync_fn)(args, BroadcastSink { tx }).await
    }).await {
        Some((id, _tx)) => (StatusCode::ACCEPTED, Json(json!({"job_id": id, "status": "started"}))).into_response(),
        None => (
            StatusCode::CONFLICT,
            Json(json!({"error": "another mutating job is running", "current_kind": s.executor.registry.current_active_kind().await.map(|k| k.as_str())})),
        ).into_response(),
    }
}
```

ingest, wiki_update도 동일 패턴.

### 4. GET /api/jobs/:id/stream — SSE 핸들러

```rust
use axum::response::sse::{Event, Sse, KeepAlive};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;

async fn api_job_stream(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let registry = s.executor.registry.clone();
    let initial_state = registry.get(&id).await;

    if initial_state.is_none() {
        // 메모리에 없으면 DB에서 조회 (이미 완료/만료)
        if let Some(row) = s.executor.db.lock().ok().and_then(|db| db.get_job(&id).ok().flatten()) {
            return (StatusCode::OK, Json(row)).into_response();
        }
        return (StatusCode::NOT_FOUND, Json(json!({"error": "job not found"}))).into_response();
    }

    let receiver = match registry.subscribe(&id).await {
        Some(r) => r,
        None => return (StatusCode::GONE, Json(json!({"error": "job already evicted"}))).into_response(),
    };

    let stream = futures_util::stream::unfold(receiver, |mut rx| async move {
        match rx.recv().await {
            Ok(event) => {
                let json = serde_json::to_string(&event).unwrap_or_default();
                Some((Ok::<Event, Infallible>(Event::default().data(json)), rx))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}
```

> 첫 이벤트로 `initial_state`를 즉시 push해서 재접속 시 현재 phase를 알 수 있게 함 (위 코드는 단순화 — 실제로는 initial_state 보내고 그 다음 broadcast 구독).

### 5. GET /api/jobs (active만)

```rust
async fn api_list_jobs(
    State(s): State<AppState>,
    Query(q): Query<ListJobsQuery>,
) -> impl IntoResponse {
    let states = match q.status.as_deref() {
        Some("active") | None => s.executor.registry.list_active().await,
        Some("recent") => {
            // DB에서 최근 50개
            let db = s.executor.db.lock().unwrap();
            db.list_recent_jobs(50).unwrap_or_default()
                .into_iter()
                .map(JobState::from)
                .collect()
        }
        _ => vec![],
    };
    (StatusCode::OK, Json(json!({ "jobs": states }))).into_response()
}
```

### 6. GET /api/jobs/:id

```rust
async fn api_get_job(...) -> impl IntoResponse {
    match s.executor.registry.get(&id).await {
        Some(state) => (StatusCode::OK, Json(state)).into_response(),
        None => {
            // DB fallback
            match s.executor.db.lock().ok().and_then(|db| db.get_job(&id).ok().flatten()) {
                Some(row) => (StatusCode::OK, Json(JobState::from(row))).into_response(),
                None => (StatusCode::NOT_FOUND, Json(json!({"error": "not found"}))).into_response(),
            }
        }
    }
}
```

### 7. POST /api/jobs/:id/cancel (placeholder)

```rust
async fn api_cancel_job(...) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "cancellation not supported in P33 MVP — planned for v1.1"
    }))).into_response()
}
```

### 8. `serve.rs` 수정

```rust
pub async fn run(port: u16) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;
    // 시작 시 running/started → interrupted 일괄 갱신
    db.conn().execute(
        "UPDATE jobs SET status = 'interrupted', completed_at = datetime('now')
         WHERE status IN ('started', 'running')",
        [],
    )?;
    let cleaned = db.cleanup_old_jobs()?;
    if cleaned > 0 {
        tracing::info!("Cleaned up {} old jobs", cleaned);
    }
    let db_arc = Arc::new(std::sync::Mutex::new(db));

    let cmd_adapters = CommandAdapters {
        sync_fn: Box::new(|args, sink| Box::pin(crate::commands::sync::run_with_progress(args, sink))),
        ingest_fn: Box::new(...),
        wiki_fn: Box::new(...),
    };
    let executor = Arc::new(JobExecutor::new(db_arc.clone(), cmd_adapters));

    let config = Config::load_or_default();
    // ... (기존 search 셋업)

    start_rest_server(db_arc, search, vault_path, port, executor).await
}
```

`start_rest_server` 시그니처 확장 — `executor: Arc<JobExecutor>` 추가.

### 9. 통합 테스트

`crates/secall-core/tests/jobs_rest.rs` 신규:
- POST /api/commands/sync → 200 + job_id 반환
- 동시 POST → 두 번째 409
- GET /api/jobs/:id → 상태 조회
- DB에 완료 jobs 기록 확인

axum::Router를 직접 호출 (TestServer 또는 hyper::body로). SearchEngine 셋업이 부담스러우면 mock 또는 NoopExecutor로 대체.

## Dependencies

- Task 03 완료 (`run_with_progress` 함수, `CommandAdapters` 정의)
- 외부 crate: `futures-util` 이미 있음. axum 0.8 SSE 네이티브

## Verification

```bash
# 1. 컴파일
cargo check --all-targets

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 통합 테스트
cargo test -p secall-core --test jobs_rest

# 4. 전체 회귀
cargo test --all

# 5. 라이브 검증
cargo build --release -p secall
./target/release/secall serve --port 18092 &
SP=$!
sleep 3

# Sync 시작
JOB=$(curl -s -X POST http://127.0.0.1:18092/api/commands/sync -H "Content-Type: application/json" -d '{"local_only":true,"dry_run":true}' | jq -r '.job_id')
echo "job_id: $JOB"

# 상태 조회
curl -s "http://127.0.0.1:18092/api/jobs/$JOB" | jq .

# 동시 호출 → 409
curl -s -X POST http://127.0.0.1:18092/api/commands/sync -H "Content-Type: application/json" -d '{"dry_run":true,"local_only":true}' -w "HTTP=%{http_code}\n"

# SSE 스트림 (3초간)
timeout 3 curl -N "http://127.0.0.1:18092/api/jobs/$JOB/stream" || true

kill $SP 2>/dev/null
```

## Risks

- **AppState 변경**: 기존 11개 핸들러 모두 시그니처 갱신 필요. 큰 diff. 또는 nested state 사용
- **SSE 첫 이벤트 누락**: 구독자가 connect 전에 phase_start 이벤트 발생 시 놓침. 첫 이벤트로 `initial_state` 즉시 push 권장
- **broadcast Lagged**: 구독자가 너무 느리면 Lagged 에러. SSE 스트림 라이프타임 동안 KeepAlive로 client 살아있게 유지
- **db.lock() in async**: tokio 환경에서 std::Mutex::lock()은 sync — 짧은 시간이라 문제 없지만, 향후 tokio Mutex로 교체 검토
- **시작 시 interrupted 갱신**: 모든 running/started를 interrupted로 — 의도와 일치하지만 정확한 catch_panic 등 edge case 있음
- **cancel placeholder**: 501 명시. 클라이언트에서 호출 시 적절히 처리해야 함 (Task 06에서)

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/`, `src/jobs/` — Task 01, 02
- `crates/secall/src/commands/sync.rs`, `ingest.rs`, `wiki.rs` 본체 — Task 03 (`run_with_progress` 추가)
- `web/` — Task 05, 06, 07
- 기존 11개 REST 엔드포인트 시그니처/동작 — 보존 (Obsidian 호환)
