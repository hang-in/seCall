//! P33 Task 01 — 백그라운드 Job 시스템 코어.
//!
//! 구성:
//! - `types`: `JobKind`, `JobStatus`, `JobState`, `ProgressEvent`
//! - `registry`: 메모리 상태 + SSE broadcast 채널
//! - `executor`: 단일 큐 spawn + DB 영속화
//!
//! REST/CLI에서 mutating 작업을 시작할 때 `JobExecutor::try_spawn`을 호출하면
//! 단일 큐 정책에 의해 거절될 수 있다 (-> 409 Conflict).

pub mod adapters;
pub mod executor;
pub mod registry;
pub mod types;

pub use adapters::{AdapterFn, CommandAdapters};
pub use executor::JobExecutor;
pub use registry::JobRegistry;
pub use types::{JobKind, JobState, JobStatus, ProgressEvent};

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Job 본체 코드(예: sync, ingest)가 progress를 보고할 때 사용하는 추상화.
///
/// Task 02에서 Job 본체 함수들이 `&dyn ProgressSink`를 받아 구현체와 무관하게
/// progress를 보고할 수 있도록 한다.
#[async_trait]
pub trait ProgressSink: Send + Sync {
    async fn phase_start(&self, phase: &str);
    async fn message(&self, text: &str);
    async fn progress(&self, ratio: f32);
    async fn phase_complete(&self, phase: &str, result: Option<serde_json::Value>);

    /// P36 Task 01 — cancellation 폴링용 hook.
    ///
    /// 어댑터(Task 02)가 안전 지점마다 호출하여 자발적으로 종료할 수 있도록 한다.
    /// 호출 비용은 atomic load 한 번이므로 hot loop 안에서도 자유롭게 호출 가능하다.
    /// 기존 sink 구현체를 깨지 않기 위해 default `false` 를 반환한다.
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// `broadcast::Sender<ProgressEvent>`를 `ProgressSink`로 어댑팅.
///
/// Executor가 spawn한 Job 본체에 넘겨주는 기본 구현. 구독자가 없으면
/// `send`가 Err를 반환하지만 의도된 동작이므로 무시한다.
///
/// P36 Task 01 — Job 별 `CancellationToken` 을 보유하고, 어댑터가
/// `is_cancelled()` 로 폴링할 수 있도록 한다.
pub struct BroadcastSink {
    pub tx: broadcast::Sender<ProgressEvent>,
    pub cancel_token: CancellationToken,
}

impl BroadcastSink {
    pub fn new(tx: broadcast::Sender<ProgressEvent>, cancel_token: CancellationToken) -> Self {
        Self { tx, cancel_token }
    }
}

#[async_trait]
impl ProgressSink for BroadcastSink {
    async fn phase_start(&self, phase: &str) {
        let _ = self.tx.send(ProgressEvent::PhaseStart {
            phase: phase.to_string(),
        });
    }

    async fn message(&self, text: &str) {
        let _ = self.tx.send(ProgressEvent::Message {
            text: text.to_string(),
        });
    }

    async fn progress(&self, ratio: f32) {
        let _ = self.tx.send(ProgressEvent::Progress { ratio });
    }

    async fn phase_complete(&self, phase: &str, result: Option<serde_json::Value>) {
        let _ = self.tx.send(ProgressEvent::PhaseComplete {
            phase: phase.to_string(),
            result,
        });
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}
