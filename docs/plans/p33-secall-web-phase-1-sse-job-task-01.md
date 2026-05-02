---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — `Job` 코어 모듈 (registry / executor / 단일 큐)

## Changed files

신규:
- `crates/secall-core/src/jobs/mod.rs` — public API + 타입 정의
- `crates/secall-core/src/jobs/registry.rs` — `JobRegistry` (메모리 상태)
- `crates/secall-core/src/jobs/executor.rs` — `JobExecutor` (spawn + 단일 큐)
- `crates/secall-core/src/jobs/types.rs` — `JobKind`, `JobStatus`, `JobState`, `Phase`, `ProgressEvent` 등

수정:
- `crates/secall-core/src/lib.rs` — `pub mod jobs;` 추가
- `crates/secall-core/Cargo.toml` — `uuid = { version = "1", features = ["v4"] }`, `tokio` 이미 있음, `tokio-util` 이미 있음

## Change description

### 1. 의존성 추가

`Cargo.toml` (workspace):
```toml
uuid = { version = "1", features = ["v4", "serde"] }
```

`crates/secall-core/Cargo.toml`:
```toml
uuid = { workspace = true }
```

### 2. `jobs/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Sync,
    Ingest,
    WikiUpdate,
}

impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::Sync => "sync",
            JobKind::Ingest => "ingest",
            JobKind::WikiUpdate => "wiki_update",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sync" => Some(JobKind::Sync),
            "ingest" => Some(JobKind::Ingest),
            "wiki_update" => Some(JobKind::WikiUpdate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Started,      // 큐 진입 또는 spawn 직후
    Running,      // 실제 phase 실행 중
    Completed,
    Failed,
    Interrupted,  // 서버 재시작 등으로 중단
}

#[derive(Debug, Clone, Serialize)]
pub struct JobState {
    pub id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub current_phase: Option<String>,    // 예: "pull", "reindex", "ingest", "push"
    pub progress: Option<f32>,            // 0.0~1.0 (선택)
    pub message: Option<String>,          // 최근 로그 한 줄
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Job 실행 중 progress reporter가 발행하는 이벤트.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressEvent {
    PhaseStart { phase: String },
    Message { text: String },
    Progress { ratio: f32 },
    PhaseComplete { phase: String, result: Option<serde_json::Value> },
    Done { result: serde_json::Value },
    Failed { error: String, partial_result: Option<serde_json::Value> },
}
```

### 3. `jobs/registry.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::types::{JobKind, JobState, JobStatus, ProgressEvent};

const BROADCAST_BUFFER: usize = 64;

/// 메모리 상의 Job 상태 + SSE 구독자 broadcast.
#[derive(Clone)]
pub struct JobRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

struct RegistryInner {
    states: HashMap<String, JobState>,
    senders: HashMap<String, broadcast::Sender<ProgressEvent>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                states: HashMap::new(),
                senders: HashMap::new(),
            })),
        }
    }

    pub async fn register(&self, state: JobState) -> broadcast::Sender<ProgressEvent> {
        let mut inner = self.inner.write().await;
        let (tx, _) = broadcast::channel(BROADCAST_BUFFER);
        inner.states.insert(state.id.clone(), state);
        let id = inner.states.values().last().map(|s| s.id.clone()).unwrap_or_default();
        let _ = id;
        let id_owned = inner.states.keys().last().cloned().unwrap_or_default();
        inner.senders.insert(id_owned.clone(), tx.clone());
        tx
    }

    /// 현재 실행 중(running/started) job 중 mutating 종류가 있으면 그 종류 반환.
    pub async fn current_active_kind(&self) -> Option<JobKind> {
        let inner = self.inner.read().await;
        inner.states.values()
            .find(|s| matches!(s.status, JobStatus::Started | JobStatus::Running))
            .map(|s| s.kind)
    }

    pub async fn get(&self, id: &str) -> Option<JobState> {
        self.inner.read().await.states.get(id).cloned()
    }

    pub async fn list_active(&self) -> Vec<JobState> {
        self.inner.read().await.states.values()
            .filter(|s| matches!(s.status, JobStatus::Started | JobStatus::Running))
            .cloned()
            .collect()
    }

    pub async fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<ProgressEvent>> {
        self.inner.read().await.senders.get(id).map(|tx| tx.subscribe())
    }

    pub async fn update<F: FnOnce(&mut JobState)>(&self, id: &str, f: F) {
        let mut inner = self.inner.write().await;
        if let Some(s) = inner.states.get_mut(id) {
            f(s);
        }
    }

    /// 완료된 Job을 메모리에서 제거 (Done/Failed/Interrupted은 일정 시간 후 제거).
    pub async fn evict(&self, id: &str) {
        let mut inner = self.inner.write().await;
        inner.states.remove(id);
        inner.senders.remove(id);
    }
}
```

### 4. `jobs/executor.rs`

```rust
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use super::registry::JobRegistry;
use super::types::{JobKind, JobState, JobStatus, ProgressEvent};
use crate::store::Database;

/// 단일 mutating job만 동시에 허용. read 작업은 Executor 거치지 않음.
#[derive(Clone)]
pub struct JobExecutor {
    pub registry: JobRegistry,
    db: Arc<std::sync::Mutex<Database>>,
    /// 단일 큐 lock. mutating job 시작 시 try_lock하여 점유.
    write_lock: Arc<Mutex<()>>,
}

impl JobExecutor {
    pub fn new(db: Arc<std::sync::Mutex<Database>>) -> Self {
        Self {
            registry: JobRegistry::new(),
            db,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 새 job 시작 시도. 같은 종류 중복 또는 다른 mutating 실행 중이면 None 반환 (-> 409).
    /// 성공 시 (job_id, broadcast::Sender) 반환.
    pub async fn try_spawn<F, Fut>(
        &self,
        kind: JobKind,
        metadata: Option<serde_json::Value>,
        f: F,
    ) -> Option<(String, broadcast::Sender<ProgressEvent>)>
    where
        F: FnOnce(broadcast::Sender<ProgressEvent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        // 단일 큐: 다른 mutating job이 실행 중이면 거부
        if self.registry.current_active_kind().await.is_some() {
            return None;
        }

        let id = Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        let state = JobState {
            id: id.clone(),
            kind,
            status: JobStatus::Started,
            started_at: started_at.clone(),
            completed_at: None,
            current_phase: None,
            progress: None,
            message: None,
            error: None,
            result: None,
            metadata: metadata.clone(),
        };

        // DB에 기록
        if let Ok(db) = self.db.lock() {
            let _ = db.insert_job(&id, kind.as_str(), metadata.as_ref());
        }

        let tx = self.registry.register(state).await;

        // spawn
        let registry = self.registry.clone();
        let db = self.db.clone();
        let tx_clone = tx.clone();
        let id_clone = id.clone();
        let lock = self.write_lock.clone();
        tokio::spawn(async move {
            let _guard = lock.lock().await; // 단일 큐 lock

            registry.update(&id_clone, |s| s.status = JobStatus::Running).await;

            let result = f(tx_clone.clone()).await;

            // 완료 처리
            let (status, error_text, result_json) = match &result {
                Ok(v) => (JobStatus::Completed, None, Some(v.clone())),
                Err(e) => (JobStatus::Failed, Some(e.to_string()), None),
            };
            let completed_at = chrono::Utc::now().to_rfc3339();
            registry.update(&id_clone, |s| {
                s.status = status;
                s.completed_at = Some(completed_at.clone());
                s.error = error_text.clone();
                s.result = result_json.clone();
            }).await;

            // DB persist
            if let Ok(db) = db.lock() {
                let _ = db.complete_job(
                    &id_clone,
                    match status { JobStatus::Completed => "completed", _ => "failed" },
                    result_json.as_ref(),
                    error_text.as_deref(),
                );
            }

            // 마지막 이벤트 broadcast (구독자가 끊었을 수도 있어 무시)
            let final_event = match &result {
                Ok(v) => ProgressEvent::Done { result: v.clone() },
                Err(e) => ProgressEvent::Failed {
                    error: e.to_string(),
                    partial_result: None,
                },
            };
            let _ = tx_clone.send(final_event);

            // 메모리에서 일정 시간 후 evict (재접속 가능하도록 5분 유지)
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                registry.evict(&id_clone).await;
            });
        });

        Some((id, tx))
    }
}
```

> `try_spawn`이 None을 반환하면 REST 핸들러는 409 Conflict로 응답.

### 5. 시작 시 running → interrupted 보정 (선택)

서버 시작 시 (Task 04에서 `start_rest_server` 또는 `serve.rs`에서) 다음을 수행:
```rust
db.conn().execute(
    "UPDATE jobs SET status = 'interrupted', completed_at = datetime('now')
     WHERE status IN ('started', 'running')",
    [],
)?;
```
Task 02 본문에는 함수 정의만 추가하고, 실제 호출은 Task 04 (또는 `serve.rs`)에서.

### 6. 단위 테스트

`crates/secall-core/src/jobs/registry.rs`, `executor.rs`에 작은 단위 테스트:
- registry register/get/list_active/evict
- executor try_spawn 성공
- executor 동시 호출 → 두 번째는 None
- progress event broadcast → subscribe 가능

## Dependencies

- Task 01 완료 (`jobs` 테이블 + insert_job/complete_job 메서드)
- 신규 외부 crate: `uuid 1.x`

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core --all-features

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 jobs 모듈 테스트
cargo test -p secall-core --lib jobs::

# 4. 전체 테스트 회귀
cargo test --all
```

## Risks

- **broadcast 채널 lag**: 구독자가 늦게 connect하면 PhaseStart를 놓칠 수 있음. 현재 `BROADCAST_BUFFER=64`로 충분하지만, 재접속 시 마지막 상태는 `JobRegistry::get()`으로 별도 fetch 필요 (Task 04에서 처리)
- **단일 큐 lock**: tokio::sync::Mutex는 fair queueing 아님. 사용자 입장에서 실행 순서 보장 안 됨 (보통 문제 안 됨)
- **메모리 evict 5분**: 완료 직후 5분 내에 다시 GET 가능. 그 후엔 DB로만 조회. 적절한 타협
- **panic 안전성**: spawn된 future가 panic하면 lock guard drop으로 lock 해제됨. 다만 registry/db 상태는 갱신 안 됨 → catch_unwind 추가 검토 (v1.1)
- **server 재시작 시 running 보정**: Task 02 본문은 schema 추가만. 실제 보정 호출은 Task 04 (serve 진입점)
- **`if let Ok(db) = self.db.lock()`**: lock poisoned 시 silent fail. 로그는 남겨야 함 (`tracing::error!`)

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/` — Task 01에서 schema/repo 정의 후, Task 02는 사용만
- `crates/secall-core/src/mcp/` — Task 04
- `crates/secall/src/commands/` — Task 03, 08
- `web/` — Task 05, 06, 07
