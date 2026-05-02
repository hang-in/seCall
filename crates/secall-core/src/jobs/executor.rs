//! P33 Task 01 — JobExecutor: 단일 큐 정책 + spawn + DB 영속화.
//!
//! 동시에 mutating job은 1개만 허용. 두 번째 요청은 None 반환 → REST 핸들러가 409 응답.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::adapters::CommandAdapters;
use super::registry::JobRegistry;
use super::types::{JobKind, JobState, JobStatus, ProgressEvent};
use crate::store::Database;

/// 단일 mutating job만 동시에 허용. read 작업은 Executor 거치지 않는다.
///
/// `db`/`adapters`는 REST 핸들러(Task 03)가 직접 참조하므로 `pub`이다.
#[derive(Clone)]
pub struct JobExecutor {
    pub registry: JobRegistry,
    pub db: Arc<std::sync::Mutex<Database>>,
    /// 명령 어댑터 묶음. REST 핸들러가 args/sink와 함께 호출.
    /// `with_adapters` 생성자로 주입되며, `new`로 만든 인스턴스에서는 None.
    pub adapters: Option<Arc<CommandAdapters>>,
    /// spawn-gate: try_spawn 진입 직렬화. active 체크 + register를 원자적으로 보호.
    /// 이 락이 없으면 두 동시 요청이 모두 idle을 관측해서 동시 spawn될 수 있다 (P33 review-r1 finding).
    spawn_gate: Arc<Mutex<()>>,
    /// 단일 큐 lock. spawn된 future가 실제 실행 직전에 획득.
    write_lock: Arc<Mutex<()>>,
}

impl JobExecutor {
    pub fn new(db: Arc<std::sync::Mutex<Database>>) -> Self {
        Self {
            registry: JobRegistry::new(),
            db,
            adapters: None,
            spawn_gate: Arc::new(Mutex::new(())),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// `CommandAdapters`를 함께 주입한 생성자. REST 서버(serve.rs)가 사용.
    pub fn with_adapters(db: Arc<std::sync::Mutex<Database>>, adapters: CommandAdapters) -> Self {
        Self {
            registry: JobRegistry::new(),
            db,
            adapters: Some(Arc::new(adapters)),
            spawn_gate: Arc::new(Mutex::new(())),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 새 job 시작 시도.
    ///
    /// 다른 mutating job이 실행 중이면 None 반환 (-> REST 핸들러는 409).
    /// 성공 시 (job_id, broadcast::Sender) 반환. Sender는 progress reporter용.
    ///
    /// `f`는 spawn된 task에서 실행되며, broadcast Sender를 받아 ProgressEvent를 발행한다.
    /// 반환 `Ok(value)` → Done event + DB completed, `Err(e)` → Failed event + DB failed.
    /// P36 Task 01 — `f` 시그니처에 `CancellationToken` 추가.
    /// 호출자(REST 핸들러)는 이 토큰을 `BroadcastSink::new` 에 그대로 넘겨
    /// 어댑터가 `is_cancelled()` 폴링할 수 있게 한다.
    pub async fn try_spawn<F, Fut>(
        &self,
        kind: JobKind,
        metadata: Option<serde_json::Value>,
        f: F,
    ) -> Option<(String, broadcast::Sender<ProgressEvent>)>
    where
        F: FnOnce(broadcast::Sender<ProgressEvent>, CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        // P33 review-r1 fix: spawn-gate로 active 체크 + register를 원자적으로 묶는다.
        // 이전 구현은 check와 register 사이에 다른 요청이 끼어들 수 있어 두 spawn이
        // 모두 성공할 수 있었다. 이 락은 spawn 진입 직렬화 전용이며, 실제 작업 실행은
        // tokio::spawn 안의 write_lock이 담당한다. (즉 spawn_gate는 짧은 비동기 critical section)
        let _gate = self.spawn_gate.lock().await;

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

        // DB에 시작 기록
        match self.db.lock() {
            Ok(db) => {
                if let Err(e) = db.insert_job(&id, kind.as_str(), metadata.as_ref()) {
                    tracing::error!(job_id = %id, kind = kind.as_str(), error = %e, "insert_job failed");
                }
            }
            Err(e) => {
                tracing::error!(job_id = %id, error = %e, "db mutex poisoned at insert_job");
            }
        }

        // P36 Task 01 — 이 job 전용 CancellationToken.
        // registry 와 spawned task 가 동일 토큰을 공유한다.
        // 어댑터 sink (BroadcastSink) 도 이 토큰을 받아 `is_cancelled()` 폴링에 사용.
        let cancel_token = CancellationToken::new();
        let tx = self.registry.register(state, cancel_token.clone()).await;

        // spawn
        let registry = self.registry.clone();
        let db = self.db.clone();
        let tx_clone = tx.clone();
        let id_clone = id.clone();
        let lock = self.write_lock.clone();
        let cancel_token_spawn = cancel_token.clone();
        tokio::spawn(async move {
            let _guard = lock.lock().await; // 단일 큐 lock

            registry
                .update(&id_clone, |s| s.status = JobStatus::Running)
                .await;

            // P36 Task 01 — 사용자 클로저를 cancel 신호와 race.
            // cancel 분기 진입 시 user future 는 drop 되며 어댑터 자체 cleanup 책임.
            let result = tokio::select! {
                biased;
                _ = cancel_token_spawn.cancelled() => {
                    Err(anyhow::anyhow!("cancelled by user"))
                }
                r = f(tx_clone.clone(), cancel_token_spawn.clone()) => r,
            };

            // 완료 처리.
            // P36 Task 01 — cancel race: 어댑터가 token cancel 직전에 Ok 반환 가능.
            // 일관성을 위해 token 이 trigger 되었으면 status 를 Interrupted 로 강제하고
            // error 메시지는 "cancelled by user" 로 통일한다.
            //
            // P36 rework — partial_result 보존: 어댑터 계약상 cancel 시
            // `Ok(partial_outcome)` 으로 반환할 수 있으므로, was_cancelled 라도
            // 그 값을 result_json 에 보존해 registry/DB/SSE 양쪽에 노출한다.
            // select! 가 cancel 분기로 진입한 경우 result 는 Err → partial 없음.
            let was_cancelled = cancel_token_spawn.is_cancelled();
            let (status, error_text, result_json) = if was_cancelled {
                let partial = result.as_ref().ok().cloned();
                (
                    JobStatus::Interrupted,
                    Some("cancelled by user".to_string()),
                    partial,
                )
            } else {
                match &result {
                    Ok(v) => (JobStatus::Completed, None, Some(v.clone())),
                    Err(e) => (JobStatus::Failed, Some(e.to_string()), None),
                }
            };
            let completed_at = chrono::Utc::now().to_rfc3339();
            let error_text_clone = error_text.clone();
            let result_json_clone = result_json.clone();
            registry
                .update(&id_clone, |s| {
                    s.status = status;
                    s.completed_at = Some(completed_at.clone());
                    s.error = error_text_clone;
                    s.result = result_json_clone;
                })
                .await;

            // DB persist
            // P36 Task 01 — Interrupted 도 DB schema 에 별도 column 이 없으므로
            // 기존 string mapping ("interrupted") 을 그대로 사용.
            let db_status = match status {
                JobStatus::Completed => "completed",
                JobStatus::Interrupted => "interrupted",
                _ => "failed",
            };
            match db.lock() {
                Ok(db) => {
                    if let Err(e) = db.complete_job(
                        &id_clone,
                        db_status,
                        result_json.as_ref(),
                        error_text.as_deref(),
                    ) {
                        tracing::error!(job_id = %id_clone, error = %e, "complete_job failed");
                    }
                }
                Err(e) => {
                    tracing::error!(job_id = %id_clone, error = %e, "db mutex poisoned at complete_job");
                }
            }

            // 마지막 이벤트 broadcast (구독자가 끊었을 수도 있어 무시)
            // P36 Task 01 — cancelled 인 경우 Failed 로 통일.
            // P36 rework — partial_result 보존: 어댑터가 cancel 폴링 시 반환한
            // Ok(partial_outcome) 의 JSON 을 SSE 구독자에게도 그대로 전달.
            let final_event = if was_cancelled {
                ProgressEvent::Failed {
                    error: "cancelled by user".to_string(),
                    partial_result: result_json.clone(),
                }
            } else {
                match &result {
                    Ok(v) => ProgressEvent::Done { result: v.clone() },
                    Err(e) => ProgressEvent::Failed {
                        error: e.to_string(),
                        partial_result: None,
                    },
                }
            };
            let _ = tx_clone.send(final_event);

            // 5분 후 메모리에서 evict (재접속 가능 기간)
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                registry.evict(&id_clone).await;
            });
        });

        Some((id, tx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_executor() -> (JobExecutor, TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).expect("open db");
        let exec = JobExecutor::new(Arc::new(std::sync::Mutex::new(db)));
        (exec, dir)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn try_spawn_returns_some_when_idle() {
        let (exec, _dir) = make_executor();
        let res = exec
            .try_spawn(JobKind::Sync, None, |_tx, _cancel| async {
                Ok(serde_json::json!({"ok": true}))
            })
            .await;
        assert!(res.is_some(), "first spawn must succeed");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn try_spawn_second_call_returns_none_while_running() {
        let (exec, _dir) = make_executor();
        // 첫 번째 job은 일부러 충분히 오래 걸리게 만든다.
        let first = exec
            .try_spawn(JobKind::Sync, None, |_tx, _cancel| async {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                Ok(serde_json::json!({"ok": true}))
            })
            .await;
        assert!(first.is_some(), "first spawn must succeed");

        // 첫 번째가 status=Running으로 바뀌도록 약간 대기.
        // (write_lock 획득 → registry.update까지 비동기적으로 진행됨)
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let second = exec
            .try_spawn(JobKind::Ingest, None, |_tx, _cancel| async {
                Ok(serde_json::json!({"never": true}))
            })
            .await;
        assert!(
            second.is_none(),
            "second spawn must be rejected while another is running"
        );
    }

    /// P33 review-r1 finding: spawn-gate가 race condition을 막는지 검증.
    /// 동시에 N개의 try_spawn을 호출하면 정확히 1개만 Some, 나머지는 None이어야 한다.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn try_spawn_concurrent_calls_serialize_via_spawn_gate() {
        let (exec, _dir) = make_executor();
        let n = 10;
        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let exec = exec.clone();
            handles.push(tokio::spawn(async move {
                exec.try_spawn(JobKind::Sync, None, |_tx, _cancel| async {
                    // 첫 spawn이 충분히 오래 살아있도록
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    Ok(serde_json::json!({"ok": true}))
                })
                .await
                .is_some()
            }));
        }
        let mut some_count = 0;
        for h in handles {
            if h.await.unwrap() {
                some_count += 1;
            }
        }
        assert_eq!(
            some_count, 1,
            "exactly one concurrent try_spawn must succeed (got {some_count})"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn progress_event_is_broadcast_to_subscriber() {
        let (exec, _dir) = make_executor();
        let (id, tx) = exec
            .try_spawn(JobKind::Sync, None, |tx, _cancel| async move {
                tx.send(ProgressEvent::PhaseStart {
                    phase: "pull".into(),
                })
                .ok();
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                Ok(serde_json::json!({"done": true}))
            })
            .await
            .expect("spawn ok");

        // tx는 try_spawn이 반환한 동일 채널이므로 이를 통해 직접 subscribe 가능하지만,
        // registry 경유 subscribe 경로도 검증한다.
        let mut rx = exec
            .registry
            .subscribe(&id)
            .await
            .expect("subscribe must succeed before evict");

        // Done event까지 적어도 하나의 이벤트는 받을 수 있어야 한다.
        let received = tokio::time::timeout(std::time::Duration::from_secs(2), async move {
            let mut events = Vec::new();
            while let Ok(ev) = rx.recv().await {
                let is_done = matches!(
                    ev,
                    ProgressEvent::Done { .. } | ProgressEvent::Failed { .. }
                );
                events.push(ev);
                if is_done {
                    break;
                }
            }
            events
        })
        .await
        .expect("must not time out");

        // tx가 drop되지 않도록 유지
        drop(tx);

        assert!(
            received
                .iter()
                .any(|e| matches!(e, ProgressEvent::Done { .. })),
            "Done event must be received: {received:?}"
        );
    }

    /// P36 Task 01 — cancel 호출 시 status 가 Interrupted 로 전이되고
    /// SSE 채널로 Failed 이벤트가 발행되는지 검증.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancel_marks_job_as_interrupted_and_emits_failed_event() {
        let (exec, _dir) = make_executor();
        let (id, _tx) = exec
            .try_spawn(JobKind::Sync, None, |_tx, _cancel| async move {
                // 충분히 긴 작업 — cancel select 분기로 빠지도록.
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                Ok(serde_json::json!({"never": true}))
            })
            .await
            .expect("spawn ok");

        // 구독자 등록 (cancel 전에 미리 붙여야 final event 수신 보장).
        let mut rx = exec
            .registry
            .subscribe(&id)
            .await
            .expect("subscribe must succeed");

        // spawn task 가 write_lock 획득 후 select 진입할 때까지 잠깐 대기.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(exec.registry.cancel(&id).await, "cancel must return true");

        // 1초 안에 Failed 이벤트 수신.
        let got_failed = tokio::time::timeout(std::time::Duration::from_secs(1), async move {
            loop {
                match rx.recv().await {
                    Ok(ProgressEvent::Failed { error, .. }) => {
                        return Some(error);
                    }
                    Ok(_) => continue,
                    Err(_) => return None,
                }
            }
        })
        .await
        .expect("must not time out");

        assert_eq!(
            got_failed.as_deref(),
            Some("cancelled by user"),
            "Failed event must carry cancelled-by-user error"
        );

        // status 도 Interrupted 로 수렴해야 한다.
        // spawn task 의 update 가 끝날 때까지 약간의 settle time 부여.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            let st = exec.registry.get(&id).await.expect("state must exist");
            if st.status == JobStatus::Interrupted {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("status did not converge to Interrupted: {:?}", st.status);
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// P36 Task 01 — 미등록 job id 로 cancel 호출 시 false.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancel_unknown_job_returns_false() {
        let (exec, _dir) = make_executor();
        let cancelled = exec.registry.cancel("does-not-exist").await;
        assert!(!cancelled, "unknown id must return false");
    }
}
