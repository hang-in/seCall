//! P38 Task 00 — axum Router 통합 테스트용 공유 fixture.
//!
//! Cargo integration tests 는 각 `tests/*.rs` 가 별도 크레이트로 컴파일되므로,
//! 본 모듈을 사용하려는 통합 테스트 파일은 다음 패턴을 따른다:
//!
//! ```text
//! mod common;
//! use common::*;
//! ```
//!
//! 본 헬퍼는 `rest_router(...)` 를 in-process 로 빌드하고 `tower::ServiceExt`
//! 의 `oneshot` 으로 단발 요청/응답을 검증한다. HTTP listener 가 없으므로 포트
//! 충돌이 없고 격리도 안전하다.
//!
//! 외부 LLM/네트워크 호출이 발생하지 않도록 `make_fake_adapters()` 가
//! sync/ingest/wiki/graph_rebuild fn 모두 즉시 dummy outcome 을 반환한다.
//! (기존 `tests/jobs_rest.rs::make_adapters` 와 동일한 구조의 copy.)

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use chrono::TimeZone;
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use secall_core::ingest::{AgentKind, Session, TokenUsage};
use secall_core::jobs::{BroadcastSink, CommandAdapters, JobExecutor, ProgressEvent, ProgressSink};
use secall_core::mcp::SeCallMcpServer;
use secall_core::search::{Bm25Indexer, LinderaKoTokenizer, SearchEngine};
use secall_core::store::{Database, SessionRepo};

/// REST 라우터 통합 테스트용 격리 환경.
///
/// 각 필드는 테스트가 직접 접근할 수 있도록 `pub` 으로 노출한다.
/// `_tempdir` 은 RAII drop 으로 DB 파일/디렉터리를 정리하기 위해 보관만 한다
/// (직접 사용하지 않으므로 underscore prefix).
pub struct TestEnv {
    pub _tempdir: TempDir,
    pub db: Arc<Mutex<Database>>,
    pub executor: Arc<JobExecutor>,
    pub router: Router,
}

/// 격리된 tempdir + DB v8 자동 마이그레이션 + fake adapters 주입 + axum Router 빌드.
///
/// `JobExecutor::with_adapters` 가 sync API 이지만, REST 라우터의 일부 핸들러
/// (status 등) 는 async tokio 컨텍스트를 요구하므로 본 함수도 async 로 둔다.
pub async fn make_test_env() -> TestEnv {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");
    let db = Database::open(&db_path).expect("open db (v8 migration)");
    let db_arc = Arc::new(Mutex::new(db));

    let executor = Arc::new(JobExecutor::with_adapters(
        db_arc.clone(),
        make_fake_adapters(0),
    ));

    // SearchEngine 은 BM25 only 로 두 — 벡터 인덱스는 ort/usearch 초기화가 무거움.
    let tok = LinderaKoTokenizer::new().expect("tokenizer init");
    let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);

    let vault_path: PathBuf = dir.path().join("vault");
    let server = SeCallMcpServer::new(db_arc.clone(), Arc::new(engine), vault_path);
    let router = secall_core::mcp::rest::rest_router(server, executor.clone());

    TestEnv {
        _tempdir: dir,
        db: db_arc,
        executor,
        router,
    }
}

/// 단발 HTTP 요청을 라우터에 흘려보내고 (status, json) 을 반환.
///
/// - `body` Some → `Content-Type: application/json` + JSON 직렬화 본문.
/// - 응답 본문은 항상 JSON deserialize 시도. 빈 본문이면 `Value::Null`.
/// - 실패 응답 (`{"error": ...}`) 도 동일 경로 → 호출자가 status + json 으로 분기.
///
/// axum 0.8 부터 `axum::body::to_bytes(body, limit)` 가 `BodyExt::collect` 의
/// 단순화된 wrapper 를 제공하므로 그것을 사용한다.
pub async fn send_request(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let req = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            let bytes = serde_json::to_vec(&v).expect("serialize body");
            builder.body(Body::from(bytes)).expect("build request")
        }
        None => builder.body(Body::empty()).expect("build request"),
    };

    let response = router
        .clone()
        .oneshot(req)
        .await
        .expect("router oneshot must not fail");

    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");

    let json = if body_bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
    };

    (status, json)
}

/// fake `CommandAdapters` — 실제 sync/ingest/wiki/graph 명령을 호출하지 않고
/// 즉시 dummy outcome 을 반환한다.
///
/// `tests/jobs_rest.rs::make_adapters` 의 구조를 그대로 옮긴 copy.
/// 원본 파일은 P36/P37 회귀 테스트에서 그대로 사용되므로 건드리지 않는다.
pub fn make_fake_adapters(delay_ms: u64) -> CommandAdapters {
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
                let slices = (delay_ms / 50).max(1);
                for _ in 0..slices {
                    if sink.is_cancelled() {
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

/// P32~P37 호환 minimal session row. 다른 통합 테스트의 `make_session` 패턴
/// (`tests/rest_listing.rs`) 을 따른다.
///
/// 호출자가 DB 에 직접 insert 한다 — Mutex 잠금은 호출자 책임.
pub fn insert_minimal_session(db: &Database, id: &str) {
    let session = Session {
        id: id.to_string(),
        agent: AgentKind::ClaudeCode,
        model: Some("claude-sonnet-4-6".to_string()),
        project: Some("test-proj".to_string()),
        cwd: None,
        git_branch: None,
        host: None,
        start_time: chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        end_time: None,
        turns: vec![],
        total_tokens: TokenUsage::default(),
        session_type: "interactive".to_string(),
    };
    db.insert_session(&session).expect("insert minimal session");
}
