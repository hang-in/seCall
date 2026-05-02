//! P33 Task 01 — JobRegistry: 메모리 상의 Job 상태 + SSE 구독자 broadcast.
//!
//! 모든 mutating job은 시작 시 register되어 메모리에 남고, 완료 후 일정 시간(5분)
//! 뒤 evict된다. 그 이후 GET 요청은 DB 조회로 폴백 (Task 04).

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

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
    /// P36 Task 01 — Job 별 cancel 신호 보관소.
    /// `register` 시 주입되며 `evict` 시 함께 제거된다.
    cancel_tokens: HashMap<String, CancellationToken>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                states: HashMap::new(),
                senders: HashMap::new(),
                cancel_tokens: HashMap::new(),
            })),
        }
    }

    /// 새 Job 상태 등록 + broadcast 채널 생성. tx를 반환해 progress reporter가 사용.
    ///
    /// P36 Task 01 — `cancel_token` 도 함께 보관하여 `cancel(id)` 가 동일 토큰을
    /// trigger 할 수 있도록 한다.
    pub async fn register(
        &self,
        state: JobState,
        cancel_token: CancellationToken,
    ) -> broadcast::Sender<ProgressEvent> {
        let (tx, _) = broadcast::channel(BROADCAST_BUFFER);
        let id = state.id.clone();
        let mut inner = self.inner.write().await;
        inner.senders.insert(id.clone(), tx.clone());
        inner.cancel_tokens.insert(id.clone(), cancel_token);
        inner.states.insert(id, state);
        tx
    }

    /// P36 Task 01 — Job 취소.
    ///
    /// 동작:
    /// - 미등록 id → `false`
    /// - 활성 (Started/Running) → token cancel + 즉시 status 를 Interrupted 로 전이 → `true`
    /// - 이미 종료된 job (Completed/Failed/Interrupted) → idempotent `true`
    ///
    /// 실제 spawn task 종료/SSE final event 발행은 executor 의 select 루프가 담당한다.
    pub async fn cancel(&self, id: &str) -> bool {
        let mut inner = self.inner.write().await;
        let Some(state) = inner.states.get(id) else {
            return false;
        };
        let was_active = matches!(state.status, JobStatus::Started | JobStatus::Running);

        // token cancel 은 idempotent. 등록되어 있으면 trigger.
        if let Some(token) = inner.cancel_tokens.get(id) {
            token.cancel();
        }

        if was_active {
            if let Some(s) = inner.states.get_mut(id) {
                s.status = JobStatus::Interrupted;
            }
        }
        true
    }

    /// P36 Task 01 — 외부(주로 테스트)에서 token 직접 조회.
    pub async fn token_for(&self, id: &str) -> Option<CancellationToken> {
        self.inner.read().await.cancel_tokens.get(id).cloned()
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
        inner.cancel_tokens.remove(id);
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
        let _tx = reg.register(st, CancellationToken::new()).await;
        let got = reg.get("a").await.expect("registered state must be Some");
        assert_eq!(got.id, "a");
        assert_eq!(got.kind, JobKind::Sync);
        assert_eq!(got.status, JobStatus::Started);
    }

    #[tokio::test]
    async fn list_active_filters_completed() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(
                dummy_state("running", JobKind::Sync, JobStatus::Running),
                CancellationToken::new(),
            )
            .await;
        let _ = reg
            .register(
                dummy_state("done", JobKind::Ingest, JobStatus::Completed),
                CancellationToken::new(),
            )
            .await;
        let active = reg.list_active().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "running");
    }

    #[tokio::test]
    async fn current_active_kind_picks_in_progress_only() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(
                dummy_state("c", JobKind::Ingest, JobStatus::Completed),
                CancellationToken::new(),
            )
            .await;
        assert!(reg.current_active_kind().await.is_none());

        let _ = reg
            .register(
                dummy_state("r", JobKind::Sync, JobStatus::Running),
                CancellationToken::new(),
            )
            .await;
        assert_eq!(reg.current_active_kind().await, Some(JobKind::Sync));
    }

    #[tokio::test]
    async fn evict_removes_state_and_sender() {
        let reg = JobRegistry::new();
        let _tx = reg
            .register(
                dummy_state("e", JobKind::WikiUpdate, JobStatus::Running),
                CancellationToken::new(),
            )
            .await;
        assert!(reg.subscribe("e").await.is_some());
        reg.evict("e").await;
        assert!(reg.get("e").await.is_none());
        assert!(reg.subscribe("e").await.is_none());
        assert!(reg.token_for("e").await.is_none());
    }

    #[tokio::test]
    async fn broadcast_event_received_by_subscriber() {
        let reg = JobRegistry::new();
        let tx = reg
            .register(
                dummy_state("b", JobKind::Sync, JobStatus::Running),
                CancellationToken::new(),
            )
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
            .register(
                dummy_state("u", JobKind::Sync, JobStatus::Started),
                CancellationToken::new(),
            )
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

    #[tokio::test]
    async fn cancel_unknown_returns_false() {
        let reg = JobRegistry::new();
        assert!(!reg.cancel("missing").await);
    }

    #[tokio::test]
    async fn cancel_active_marks_interrupted_and_triggers_token() {
        let reg = JobRegistry::new();
        let token = CancellationToken::new();
        let _tx = reg
            .register(
                dummy_state("x", JobKind::Sync, JobStatus::Running),
                token.clone(),
            )
            .await;
        assert!(reg.cancel("x").await);
        assert!(token.is_cancelled());
        assert_eq!(reg.get("x").await.unwrap().status, JobStatus::Interrupted);
    }

    #[tokio::test]
    async fn cancel_completed_is_idempotent_true() {
        let reg = JobRegistry::new();
        let _ = reg
            .register(
                dummy_state("done", JobKind::Sync, JobStatus::Completed),
                CancellationToken::new(),
            )
            .await;
        assert!(reg.cancel("done").await);
        // 이미 완료 상태는 그대로 유지.
        assert_eq!(reg.get("done").await.unwrap().status, JobStatus::Completed);
    }
}
