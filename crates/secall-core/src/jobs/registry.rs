//! P33 Task 01 — JobRegistry: 메모리 상의 Job 상태 + SSE 구독자 broadcast.
//!
//! 모든 mutating job은 시작 시 register되어 메모리에 남고, 완료 후 일정 시간(5분)
//! 뒤 evict된다. 그 이후 GET 요청은 DB 조회로 폴백 (Task 04).

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use super::types::{JobKind, JobState, JobStatus, ProgressEvent};

const BROADCAST_BUFFER: usize = 64;

/// 메모리 상의 Job 상태 + SSE 구독자 broadcast.
///
/// `Clone`은 Arc 복제만 수행하므로 cheap.
#[derive(Clone)]
pub struct JobRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self::new()
    }
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

    /// 새 Job 상태 등록 + broadcast 채널 생성. tx를 반환해 progress reporter가 사용.
    pub async fn register(&self, state: JobState) -> broadcast::Sender<ProgressEvent> {
        let (tx, _) = broadcast::channel(BROADCAST_BUFFER);
        let id = state.id.clone();
        let mut inner = self.inner.write().await;
        inner.senders.insert(id.clone(), tx.clone());
        inner.states.insert(id, state);
        tx
    }

    /// 현재 실행 중(started/running) job 중 하나의 종류 반환. 단일 큐 정책 체크용.
    pub async fn current_active_kind(&self) -> Option<JobKind> {
        let inner = self.inner.read().await;
        inner
            .states
            .values()
            .find(|s| matches!(s.status, JobStatus::Started | JobStatus::Running))
            .map(|s| s.kind)
    }

    pub async fn get(&self, id: &str) -> Option<JobState> {
        self.inner.read().await.states.get(id).cloned()
    }

    pub async fn list_active(&self) -> Vec<JobState> {
        self.inner
            .read()
            .await
            .states
            .values()
            .filter(|s| matches!(s.status, JobStatus::Started | JobStatus::Running))
            .cloned()
            .collect()
    }

    /// SSE 구독. 미등록 id면 None.
    pub async fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<ProgressEvent>> {
        self.inner
            .read()
            .await
            .senders
            .get(id)
            .map(|tx| tx.subscribe())
    }

    /// 상태 부분 갱신. 미등록 id면 무시.
    pub async fn update<F: FnOnce(&mut JobState)>(&self, id: &str, f: F) {
        let mut inner = self.inner.write().await;
        if let Some(s) = inner.states.get_mut(id) {
            f(s);
        }
    }

    /// 완료된 Job을 메모리에서 제거. 완료 후 5분 보존이 끝나면 호출.
    pub async fn evict(&self, id: &str) {
        let mut inner = self.inner.write().await;
        inner.states.remove(id);
        inner.senders.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_state(id: &str, kind: JobKind, status: JobStatus) -> JobState {
        JobState {
            id: id.to_string(),
            kind,
            status,
            started_at: "2026-05-02T00:00:00Z".to_string(),
            completed_at: None,
            current_phase: None,
            progress: None,
            message: None,
            error: None,
            result: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn register_then_get_returns_state() {
        let reg = JobRegistry::new();
        let st = dummy_state("a", JobKind::Sync, JobStatus::Started);
        let _tx = reg.register(st).await;
        let got = reg.get("a").await.expect("registered state must be Some");
        assert_eq!(got.id, "a");
        assert_eq!(got.kind, JobKind::Sync);
        assert_eq!(got.status, JobStatus::Started);
    }

    #[tokio::test]
    async fn list_active_filters_completed() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(dummy_state("running", JobKind::Sync, JobStatus::Running))
            .await;
        let _ = reg
            .register(dummy_state("done", JobKind::Ingest, JobStatus::Completed))
            .await;
        let active = reg.list_active().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "running");
    }

    #[tokio::test]
    async fn current_active_kind_picks_in_progress_only() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(dummy_state("c", JobKind::Ingest, JobStatus::Completed))
            .await;
        assert!(reg.current_active_kind().await.is_none());

        let _ = reg
            .register(dummy_state("r", JobKind::Sync, JobStatus::Running))
            .await;
        assert_eq!(reg.current_active_kind().await, Some(JobKind::Sync));
    }

    #[tokio::test]
    async fn evict_removes_state_and_sender() {
        let reg = JobRegistry::new();
        let _tx = reg
            .register(dummy_state("e", JobKind::WikiUpdate, JobStatus::Running))
            .await;
        assert!(reg.subscribe("e").await.is_some());
        reg.evict("e").await;
        assert!(reg.get("e").await.is_none());
        assert!(reg.subscribe("e").await.is_none());
    }

    #[tokio::test]
    async fn broadcast_event_received_by_subscriber() {
        let reg = JobRegistry::new();
        let tx = reg
            .register(dummy_state("b", JobKind::Sync, JobStatus::Running))
            .await;
        let mut rx = reg.subscribe("b").await.expect("subscribe must succeed");
        tx.send(ProgressEvent::Message {
            text: "hello".into(),
        })
        .expect("send ok");
        let ev = rx.recv().await.expect("recv ok");
        match ev {
            ProgressEvent::Message { text } => assert_eq!(text, "hello"),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_mutates_existing_state() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(dummy_state("u", JobKind::Sync, JobStatus::Started))
            .await;
        reg.update("u", |s| {
            s.status = JobStatus::Running;
            s.current_phase = Some("pull".into());
        })
        .await;
        let got = reg.get("u").await.unwrap();
        assert_eq!(got.status, JobStatus::Running);
        assert_eq!(got.current_phase.as_deref(), Some("pull"));
    }
}
