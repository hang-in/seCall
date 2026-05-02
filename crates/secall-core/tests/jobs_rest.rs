//! P33 Task 03 — Job 시스템 REST 핸들러 통합 검증.
//!
//! axum 라우터를 띄우지 않고 `JobExecutor::with_adapters` + DB를 직접 검증한다.
//! REST 엔드포인트가 호출하는 핵심 흐름(try_spawn → registry/db 영속화 →
//! broadcast 구독)을 외부 (tests/) 크레이트에서 점검한다.

use std::sync::Arc;
use std::time::Duration;

use secall_core::jobs::{
    BroadcastSink, CommandAdapters, JobExecutor, JobKind, JobStatus, ProgressEvent,
};
use secall_core::store::Database;

/// 테스트용 어댑터: sync_fn은 `delay_ms`만큼 대기 후 echo 결과 반환.
/// ingest_fn/wiki_update_fn/graph_rebuild_fn 은 즉시 Ok 반환.
///
/// P37 Task 02 — `graph_rebuild_fn` 은 `delay_ms` 만큼 대기 후 outcome 반환하며,
/// 매 50ms 단위로 `is_cancelled()` 폴링하여 cancel 시 부분 outcome 으로 early return.
fn make_adapters(delay_ms: u64) -> CommandAdapters {
    CommandAdapters {
        sync_fn: Box::new(move |val, sink: BroadcastSink| {
            Box::pin(async move {
                sink.tx
                    .send(ProgressEvent::PhaseStart {
                        phase: "test_phase".into(),
                    })
                    .ok();
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                Ok(serde_json::json!({ "echo": val }))
            })
        }),
        ingest_fn: Box::new(|_val, _sink| {
            Box::pin(async move { Ok(serde_json::json!({ "ingested": 0 })) })
        }),
        wiki_update_fn: Box::new(|_val, _sink| {
            Box::pin(async move { Ok(serde_json::json!({ "pages_written": 0 })) })
        }),
        graph_rebuild_fn: Box::new(move |val, sink: BroadcastSink| {
            Box::pin(async move {
                use secall_core::jobs::ProgressSink;
                // delay_ms 를 50ms 슬라이스로 쪼개서 cancel 폴링.
                let slices = (delay_ms / 50).max(1);
                for _ in 0..slices {
                    if sink.is_cancelled() {
                        // 부분 outcome 보존 (P36 패턴).
                        return Ok(serde_json::json!({
                            "processed": 0,
                            "succeeded": 0,
                            "failed": 0,
                            "skipped": 0,
                            "edges_added": 0,
                            "cancelled": true,
                            "args": val,
                        }));
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Ok(serde_json::json!({
                    "processed": 0,
                    "succeeded": 0,
                    "failed": 0,
                    "skipped": 0,
                    "edges_added": 0,
                    "args": val,
                }))
            })
        }),
    }
}

fn make_executor(delay_ms: u64) -> (Arc<JobExecutor>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("open db");
    let exec =
        JobExecutor::with_adapters(Arc::new(std::sync::Mutex::new(db)), make_adapters(delay_ms));
    (Arc::new(exec), dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn try_spawn_via_adapter_writes_to_db_on_complete() {
    let (exec, _dir) = make_executor(0);
    let adapters = exec.adapters.clone().expect("adapters configured");

    let args = serde_json::json!({ "local_only": true, "dry_run": true });
    let args_for_spawn = args.clone();
    let (id, _tx) = exec
        .try_spawn(
            JobKind::Sync,
            Some(args.clone()),
            move |tx, cancel_token| {
                let adapters = adapters.clone();
                let args_for_spawn = args_for_spawn.clone();
                async move {
                    let sink = BroadcastSink::new(tx, cancel_token);
                    (adapters.sync_fn)(args_for_spawn, sink).await
                }
            },
        )
        .await
        .expect("first spawn must succeed");

    // 완료까지 대기 (try_spawn은 즉시 반환, 실제 실행은 spawn된 task)
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = exec.db.lock().unwrap().get_job(&id).expect("get_job ok");
        if let Some(r) = row {
            if r.status == "completed" || r.status == "failed" {
                assert_eq!(r.status, "completed", "expected completed, got: {r:?}");
                assert!(r.completed_at.is_some());
                assert!(r.result.is_some(), "result must be persisted");
                return;
            }
        }
    }
    panic!("job did not complete within 1s");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_try_spawn_returns_none_while_running() {
    let (exec, _dir) = make_executor(500);
    let adapters = exec.adapters.clone().unwrap();

    let adapters_a = adapters.clone();
    let first = exec
        .try_spawn(JobKind::Sync, None, move |tx, cancel_token| {
            let adapters_a = adapters_a.clone();
            async move {
                let sink = BroadcastSink::new(tx, cancel_token);
                (adapters_a.sync_fn)(serde_json::json!({}), sink).await
            }
        })
        .await;
    assert!(first.is_some(), "first spawn must succeed");

    // 첫 번째가 Running 상태로 진입하길 잠시 대기.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let adapters_b = adapters.clone();
    let second = exec
        .try_spawn(JobKind::Ingest, None, move |tx, cancel_token| {
            let adapters_b = adapters_b.clone();
            async move {
                let sink = BroadcastSink::new(tx, cancel_token);
                (adapters_b.ingest_fn)(serde_json::json!({}), sink).await
            }
        })
        .await;
    assert!(
        second.is_none(),
        "second spawn must be rejected by single-queue policy"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registry_get_returns_running_state_during_execution() {
    let (exec, _dir) = make_executor(300);
    let adapters = exec.adapters.clone().unwrap();

    let adapters_a = adapters.clone();
    let (id, _tx) = exec
        .try_spawn(JobKind::Sync, None, move |tx, cancel_token| {
            let adapters_a = adapters_a.clone();
            async move {
                let sink = BroadcastSink::new(tx, cancel_token);
                (adapters_a.sync_fn)(serde_json::json!({}), sink).await
            }
        })
        .await
        .unwrap();

    // 잠시 후 registry에 Started 또는 Running 상태가 보여야 한다.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = exec.registry.get(&id).await.expect("state must exist");
    assert!(
        matches!(state.status, JobStatus::Started | JobStatus::Running),
        "unexpected status during execution: {:?}",
        state.status
    );
    assert_eq!(state.kind, JobKind::Sync);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broadcast_subscriber_receives_phase_and_done_events() {
    let (exec, _dir) = make_executor(50);
    let adapters = exec.adapters.clone().unwrap();

    let adapters_a = adapters.clone();
    let (id, _tx) = exec
        .try_spawn(JobKind::Sync, None, move |tx, cancel_token| {
            let adapters_a = adapters_a.clone();
            async move {
                let sink = BroadcastSink::new(tx, cancel_token);
                (adapters_a.sync_fn)(serde_json::json!({}), sink).await
            }
        })
        .await
        .unwrap();

    // 즉시 구독 (try_spawn 직후라 첫 PhaseStart 이벤트는 이미 발송됐을 수 있어
    // 무손실을 보장하지는 않지만 Done은 받아야 한다)
    let mut rx = exec.registry.subscribe(&id).await.expect("subscribe ok");

    let received = tokio::time::timeout(Duration::from_secs(2), async {
        let mut events = Vec::new();
        while let Ok(ev) = rx.recv().await {
            let stop = matches!(
                ev,
                ProgressEvent::Done { .. } | ProgressEvent::Failed { .. }
            );
            events.push(ev);
            if stop {
                break;
            }
        }
        events
    })
    .await
    .expect("must not time out");

    assert!(
        received
            .iter()
            .any(|e| matches!(e, ProgressEvent::Done { .. })),
        "Done event must be received: {received:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_recent_jobs_returns_persisted_rows() {
    let (exec, _dir) = make_executor(0);
    let adapters = exec.adapters.clone().unwrap();

    let adapters_a = adapters.clone();
    let (id, _tx) = exec
        .try_spawn(JobKind::Ingest, None, move |tx, cancel_token| {
            let adapters_a = adapters_a.clone();
            async move {
                let sink = BroadcastSink::new(tx, cancel_token);
                (adapters_a.ingest_fn)(serde_json::json!({"force": false}), sink).await
            }
        })
        .await
        .unwrap();

    // 완료까지 대기
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = exec.db.lock().unwrap().get_job(&id).unwrap();
        if let Some(r) = row {
            if r.status == "completed" {
                break;
            }
        }
    }

    let rows = exec.db.lock().unwrap().list_recent_jobs(50).unwrap();
    assert!(rows.iter().any(|r| r.id == id), "inserted job must appear");
    let r = rows.iter().find(|r| r.id == id).unwrap();
    assert_eq!(r.kind, "ingest");
    assert_eq!(r.status, "completed");
}

// ─── P37 Task 02 — graph rebuild adapter 통합 ───────────────────────────────

/// REST `/api/commands/graph-rebuild` 엔트리포인트가 실제로 호출하는 흐름:
/// `try_spawn(JobKind::GraphRebuild, args, |tx, token| (graph_rebuild_fn)(args, sink))`.
/// 200 + job_id 반환 후 GET /api/jobs/{id} 동등 경로 (registry/db) 로 status 추적이 가능한지 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_graph_rebuild_endpoint_starts_job() {
    let (exec, _dir) = make_executor(0);
    let adapters = exec.adapters.clone().expect("adapters configured");

    let args = serde_json::json!({ "retry_failed": true });
    let args_for_spawn = args.clone();
    let (id, _tx) = exec
        .try_spawn(
            JobKind::GraphRebuild,
            Some(args.clone()),
            move |tx, cancel_token| {
                let adapters = adapters.clone();
                let args_for_spawn = args_for_spawn.clone();
                async move {
                    let sink = BroadcastSink::new(tx, cancel_token);
                    (adapters.graph_rebuild_fn)(args_for_spawn, sink).await
                }
            },
        )
        .await
        .expect("first spawn must succeed (graph_rebuild)");

    // 잠시 후 registry 에 Started/Running 으로 등장 (GET /api/jobs/{id} 동등).
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = exec
        .registry
        .get(&id)
        .await
        .expect("graph_rebuild job must be registered");
    assert_eq!(state.kind, JobKind::GraphRebuild);

    // 완료까지 대기 → DB persist 검증.
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = exec.db.lock().unwrap().get_job(&id).expect("get_job ok");
        if let Some(r) = row {
            if r.status == "completed" || r.status == "failed" {
                assert_eq!(r.kind, "graph_rebuild");
                assert_eq!(r.status, "completed", "expected completed, got: {r:?}");
                assert!(r.result.is_some(), "result must be persisted");
                return;
            }
        }
    }
    panic!("graph_rebuild job did not complete within 2s");
}

/// P36 cancel 통합: graph_rebuild 실행 중 cancel 호출 → status=interrupted.
/// adapter 가 폴링 후 부분 outcome 을 반환하면 partial_result 도 보존된다.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_graph_rebuild_cancel_interrupts_job() {
    // 충분히 긴 delay 로 폴링 루프 진입을 보장.
    let (exec, _dir) = make_executor(2000);
    let adapters = exec.adapters.clone().expect("adapters configured");

    let args = serde_json::json!({ "all": true });
    let args_for_spawn = args.clone();
    let (id, _tx) = exec
        .try_spawn(
            JobKind::GraphRebuild,
            Some(args.clone()),
            move |tx, cancel_token| {
                let adapters = adapters.clone();
                let args_for_spawn = args_for_spawn.clone();
                async move {
                    let sink = BroadcastSink::new(tx, cancel_token);
                    (adapters.graph_rebuild_fn)(args_for_spawn, sink).await
                }
            },
        )
        .await
        .expect("first spawn must succeed");

    // Running 진입 대기.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // POST /api/jobs/{id}/cancel 동등 경로.
    assert!(exec.registry.cancel(&id).await, "cancel must succeed");

    // executor select 루프가 status 를 Interrupted 로 확정할 때까지 대기.
    let mut final_status = None;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = exec.db.lock().unwrap().get_job(&id).expect("get_job ok");
        if let Some(r) = row {
            if r.status == "interrupted" || r.status == "completed" || r.status == "failed" {
                final_status = Some(r);
                break;
            }
        }
    }
    let row = final_status.expect("job did not finalize within 2s");
    assert_eq!(
        row.status, "interrupted",
        "cancel must yield interrupted status, got {row:?}"
    );
    assert_eq!(row.kind, "graph_rebuild");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executor_without_adapters_has_none_field() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("t.db")).unwrap();
    let exec = JobExecutor::new(Arc::new(std::sync::Mutex::new(db)));
    assert!(
        exec.adapters.is_none(),
        "JobExecutor::new must not set adapters"
    );
}
