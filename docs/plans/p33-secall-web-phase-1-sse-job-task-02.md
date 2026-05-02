---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 02
parallel_group: C
depends_on: [01]
---

# Task 02 — Job → 명령 어댑터 (sync / ingest / wiki update)

## Changed files

신규:
- `crates/secall-core/src/jobs/adapters/mod.rs` — adapter 모듈 register
- `crates/secall-core/src/jobs/adapters/sync_adapter.rs` — `run_sync_job(tx, args) -> Result<Value>`
- `crates/secall-core/src/jobs/adapters/ingest_adapter.rs` — `run_ingest_job(tx, args) -> Result<Value>`
- `crates/secall-core/src/jobs/adapters/wiki_adapter.rs` — `run_wiki_update_job(tx, args) -> Result<Value>`

수정:
- `crates/secall/src/commands/sync.rs:14` — `pub async fn run(...)`이 progress reporter를 옵셔널로 받도록 시그니처 확장 (or 새 함수 `run_with_progress` 추가)
- `crates/secall/src/commands/ingest.rs:147` — `ingest_sessions`에 progress reporter 추가
- `crates/secall/src/commands/wiki.rs:9` — `run_update`에 progress reporter 추가

## Change description

### 1. 명령 함수 시그니처 확장 전략

기존 CLI는 `eprintln!`로 phase 출력. 본 task는 **신규 함수**로 분기:
- 기존 `run(...)` → 그대로 유지 (CLI 호출자 보존)
- 신규 `run_with_progress(args, &dyn ProgressSink)` 추가 — 내부 phase 진입 시 sink.report(...) 호출

`ProgressSink` trait은 `secall-core::jobs`에서 정의:
```rust
#[async_trait::async_trait]
pub trait ProgressSink: Send + Sync {
    async fn phase_start(&self, phase: &str);
    async fn message(&self, text: &str);
    async fn progress(&self, ratio: f32);
    async fn phase_complete(&self, phase: &str, result: Option<serde_json::Value>);
}
```

기존 `run`은 `NoopSink`로 호출하거나 `EprintlnSink`로 래핑. CLI 동작 보존을 위해:
```rust
pub async fn run(local_only: bool, dry_run: bool, no_wiki: bool, no_semantic: bool) -> Result<()> {
    let sink = EprintlnSink;
    run_with_progress(SyncArgs { local_only, dry_run, no_wiki, no_semantic }, &sink).await
}
```

### 2. `BroadcastSink` (Job 어댑터용)

`crates/secall-core/src/jobs/mod.rs`에 추가:
```rust
pub struct BroadcastSink {
    pub tx: tokio::sync::broadcast::Sender<ProgressEvent>,
}

#[async_trait::async_trait]
impl ProgressSink for BroadcastSink {
    async fn phase_start(&self, phase: &str) {
        let _ = self.tx.send(ProgressEvent::PhaseStart { phase: phase.to_string() });
    }
    async fn message(&self, text: &str) {
        let _ = self.tx.send(ProgressEvent::Message { text: text.to_string() });
    }
    async fn progress(&self, ratio: f32) {
        let _ = self.tx.send(ProgressEvent::Progress { ratio });
    }
    async fn phase_complete(&self, phase: &str, result: Option<serde_json::Value>) {
        let _ = self.tx.send(ProgressEvent::PhaseComplete { phase: phase.to_string(), result });
    }
}
```

### 3. `sync_adapter.rs`

```rust
pub async fn run_sync_job(
    tx: tokio::sync::broadcast::Sender<ProgressEvent>,
    args: SyncArgs,
) -> anyhow::Result<serde_json::Value> {
    let sink = BroadcastSink { tx };
    let outcome = secall::commands::sync::run_with_progress(args, &sink).await?;
    Ok(serde_json::to_value(outcome)?)
}
```

> 어댑터는 secall-core가 secall crate에 의존할 수 없으므로 (역방향), 실제 명령 함수는 secall-core에 옮기거나 trait 객체로 우회. 가장 단순: secall crate에서 어댑터 호출 (REST 핸들러를 secall에 두는 게 아니라, secall-core::jobs::adapters가 직접 sync.rs 로직을 포함하도록).

**대안 1 (권장)**: secall-core에 `core_sync.rs`, `core_ingest.rs`, `core_wiki.rs` 신규 모듈 — 실제 비즈니스 로직 이동. CLI는 wrapper만.

**대안 2**: secall-core의 `jobs::adapters`가 `dyn Fn` 또는 채널로 secall crate의 함수를 등록받음.

**선택**: 대안 1. P33 범위가 크지만 sync/ingest/wiki 비즈니스 로직을 secall-core로 옮기는 것은 long-term 정합성에 더 좋음. 본 task에서는 **sync 한정으로 mover**, ingest/wiki는 후속 subtask 또는 phase 1.1에서 점진적 이동. **단**, 본 task에서 모든 3개를 한꺼번에 옮기면 scope가 너무 커짐 → 어댑터에서 **함수 포인터/Box 클로저로 secall 함수를 호출하는 indirection** 사용:

```rust
// secall-core::jobs::adapters
pub type SyncFn = Box<dyn Fn(SyncArgs, BroadcastSink) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>> + Send + Sync>;

pub struct CommandAdapters {
    pub sync_fn: SyncFn,
    pub ingest_fn: IngestFn,
    pub wiki_fn: WikiFn,
}
```

`secall::main`에서 `CommandAdapters` 인스턴스를 만들어 `serve.rs`에 전달, `start_rest_server`가 `JobExecutor`에 등록.

> 이 방식이 가장 침습 적으면서 secall-core가 secall에 역의존하지 않음.

### 4. 명령 함수에 progress 보고 추가

`crates/secall/src/commands/sync.rs`:
```rust
pub async fn run_with_progress(args: SyncArgs, sink: &dyn ProgressSink) -> Result<SyncOutcome> {
    sink.phase_start("init").await;
    // ... (기존 로직, eprintln! 대신 sink.message)
    sink.phase_start("pull").await;
    let pull_result = ...;
    sink.phase_complete("pull", Some(json!({"new_files": pull_result.new_files}))).await;

    sink.phase_start("reindex").await;
    let reindex_result = ...;
    sink.phase_complete("reindex", Some(json!({"reindexed": reindex_result.count}))).await;

    sink.phase_start("ingest").await;
    let ingest_result = ...;
    sink.phase_complete("ingest", Some(json!({"new_sessions": ingest_result.count}))).await;

    sink.phase_start("push").await;
    let push_result = ...;
    sink.phase_complete("push", Some(json!({"pushed": push_result.commit}))).await;

    Ok(SyncOutcome { ... })
}
```

각 phase의 result는 SSE를 통해 클라이언트에 전달되어 부분 성공 표시 가능. push 실패 시 error 발생하지만 `phase_complete`로 ingest까지의 성과는 이미 전달됨.

`SyncOutcome` 구조체:
```rust
#[derive(Debug, Serialize)]
pub struct SyncOutcome {
    pub pulled: Option<usize>,
    pub reindexed: usize,
    pub ingested: usize,
    pub pushed: Option<String>,    // commit hash
}
```

ingest와 wiki도 동일 패턴으로 phase 분리.
- ingest phases: `detect`, `parse`, `classify`, `vault_write`, `index`, `vector_embed`, `semantic_extract`
- wiki phases: `prompt_build`, `llm_call`, `lint`, `merge`, `write`

세부 phase는 본 task에서 결정. progress ratio는 세션 수 기반으로 0~1 보고 가능.

### 5. JobExecutor 등록 흐름

`serve.rs` (Task 04에서 수정):
```rust
let cmd_adapters = CommandAdapters {
    sync_fn: Box::new(|args, sink| Box::pin(secall::commands::sync::run_with_progress(args, &sink))),
    ingest_fn: ...,
    wiki_fn: ...,
};
let executor = JobExecutor::new(db_arc.clone(), cmd_adapters);
```

REST 핸들러 (Task 04)에서 `executor.try_spawn(JobKind::Sync, metadata, |tx| run_sync_job(tx, args)).await`.

## Dependencies

- Task 02 완료 (`JobExecutor`, `BroadcastSink`, `ProgressSink` trait)
- 외부 crate 추가 없음

## Verification

```bash
# 1. 컴파일
cargo check --all-targets

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 어댑터 테스트 — sync run_with_progress가 EprintlnSink로 호출 시 기존 출력 동일
# (CLI 회귀 테스트는 cli_smoke 또는 새 통합 테스트로)
cargo test --all

# 4. 라이브 검증 (수동)
# 별도 터미널에서 cargo run -- sync --dry-run 실행 — 기존 출력 동일해야 함
./target/debug/secall sync --dry-run --local-only 2>&1 | head -20
```

## Risks

- **scope creep**: sync.rs를 progress sink 패턴으로 변경하면 기존 모든 phase의 eprintln!를 sink.message()로 교체 — 큰 diff. 하지만 의미적 변경은 없음
- **CLI 회귀**: `EprintlnSink`로 wrapping된 `run`이 기존 `eprintln!` 출력과 동일해야 함. cli_smoke 통과 필수
- **의존성 방향**: secall-core가 secall에 역의존하면 안 됨. `CommandAdapters` indirection 사용 (위에 명시)
- **부분 성공 직렬화**: sink가 phase_complete를 broadcast했지만 다음 phase에서 panic하면 마지막 phase 결과는 client가 봄. JobExecutor에서 spawn된 future가 panic해도 broadcast는 끊긴 상태로 남음 — Failed 이벤트 발행 보장
- **ingest 어댑터 복잡도**: ingest는 멀티 세션 → 각 세션마다 progress 보고 (ratio = i/total). 큰 데이터 셋에서 SSE 트래픽 폭증 가능 → 1초당 N개로 throttle 검토 (간단히 매 10세션마다 1회 보고)

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/` — Task 01
- `crates/secall-core/src/mcp/` — Task 04
- `web/` — Task 05, 06, 07
- 기존 `run(...)` 함수의 동작 변경 — `run_with_progress`로 이동만 가능, 동작 의미 변경 금지
