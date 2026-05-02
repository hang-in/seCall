//! P38 Task 00 — axum Router 통합 테스트 진입점.
//!
//! Section 0: 인프라 sanity (Task 00).
//! Section 1: read 라우트 11종 회귀 (Task 01).
//! Section 1A: DTO 변환 회귀 (Task 01) — 비공개 DTO 타입(`RestRecallParams` 등)에
//! 외부 통합 테스트에서 직접 접근할 수 없으므로, mode/필드 매핑이 올바르게
//! 일어났음을 라우트 응답 형태로 간접 검증한다 (production code 비수정 제약).
//!
//! Section 2 (write/commands/jobs) 는 Task 02 영역이므로 본 파일에서 다루지 않는다.

mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{insert_minimal_session, make_test_env, send_request};

// ─── Section 0: 인프라 sanity (Task 00) ───────────────────────────────────────

/// 인프라가 제대로 빌드되는지 확인. `GET /api/status` 가 200 + JSON object 를
/// 반환해야 한다.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_router_smoke_get_status() {
    let env = make_test_env().await;

    let (status, body) = send_request(&env.router, Method::GET, "/api/status", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, got {status}: {body}");
    assert!(
        body.is_object(),
        "GET /api/status must return JSON object, got: {body}"
    );
    // do_status() 응답 키 — 인프라 회귀 신호.
    assert!(
        body.get("sessions").is_some(),
        "/api/status response must contain 'sessions' key: {body}"
    );
}

// ─── Section 1: read 라우트 회귀 ──────────────────────────────────────────────

// ── 1.1 POST /api/recall ─────────────────────────────────────────────────────

/// happy path: keyword 모드 (mode 미지정 = default keyword) — 빈 DB 에서도
/// 200 + `count: 0` 반환.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recall_keyword_default_empty_db() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/recall",
        Some(json!({ "query": "anything" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["count"], 0, "empty DB → count must be 0: {body}");
    assert!(
        body.get("results").is_some(),
        "response must contain 'results' key: {body}"
    );
}

/// happy path: keyword 모드 + 빈 query 문자열 — 빈 query 도 핸들러는 통과해야
/// 한다 (BM25 가 빈 token 으로도 안전 종료). 200 + count 0 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recall_keyword_empty_query_does_not_panic() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/recall",
        Some(json!({ "query": "", "mode": "keyword" })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "empty query must not error, got {status}: {body}"
    );
    assert_eq!(body["count"], 0);
}

// ── 1.2 POST /api/get ────────────────────────────────────────────────────────

/// happy path: 존재하는 session_id → 200 + meta 필드 (id 포함).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_existing_session_returns_meta() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-get-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/get",
        Some(json!({ "session_id": "sess-get-1" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(
        body["id"], "sess-get-1",
        "id 필드는 RestGetParams 의 session_id 가 GetParams::id 로 매핑된 결과"
    );
}

/// 미존재 session_id → 500 + `error` (do_get 가 anyhow::Error 그대로 throw).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_missing_session_returns_error() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/get",
        Some(json!({ "session_id": "no-such-id" })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing session → 500, got {status}: {body}"
    );
    assert!(
        body.get("error").is_some(),
        "error body must contain 'error' key: {body}"
    );
}

/// `full=true` happy path — vault 파일이 없어도 DB turns 가 비면 content 키는
/// 없을 수도 있으므로 status + id 만 검증한다 (회귀 fragility 방지).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_full_does_not_error_when_no_vault() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-full-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/get",
        Some(json!({ "session_id": "sess-full-1", "full": true })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["id"], "sess-full-1");
}

// ── 1.3 GET /api/status (Section 0 sanity 외에 응답 키 깊이 검증) ──────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_status_returns_expected_keys() {
    let env = make_test_env().await;

    let (status, body) = send_request(&env.router, Method::GET, "/api/status", None).await;

    assert_eq!(status, StatusCode::OK);
    for key in ["sessions", "turns", "vectors", "recent_ingests"] {
        assert!(
            body.get(key).is_some(),
            "/api/status missing key '{key}': {body}"
        );
    }
}

// ── 1.4 POST /api/wiki ────────────────────────────────────────────────────────

/// happy path: vault 가 없어도 do_wiki_search 는 빈 결과 반환 (200 + count 0).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_wiki_search_empty_vault_returns_empty() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/wiki",
        Some(json!({ "query": "rust" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["count"], 0);
    assert!(body["results"].is_array());
}

// ── 1.5 GET /api/wiki/{project} ──────────────────────────────────────────────

/// 미존재 project → 404 + error (do_wiki_get 의 "not found" 메시지를 핸들러가
/// 404 로 매핑).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_wiki_get_missing_returns_404() {
    let env = make_test_env().await;

    let (status, body) =
        send_request(&env.router, Method::GET, "/api/wiki/no-such-proj", None).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "missing project → 404, got {status}: {body}"
    );
    assert!(
        body.get("error").is_some(),
        "error body must contain 'error' key: {body}"
    );
}

/// 존재하는 wiki 파일 → 200 + content 필드. tempdir 안에
/// `vault/wiki/projects/{safe_name}.md` 를 직접 만들어 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_wiki_get_existing_returns_content() {
    let env = make_test_env().await;

    // tempdir 안 vault/wiki/projects/ 디렉토리 생성 + stub md.
    let wiki_dir = env
        ._tempdir
        .path()
        .join("vault")
        .join("wiki")
        .join("projects");
    std::fs::create_dir_all(&wiki_dir).expect("mkdir vault/wiki/projects");
    let md_path = wiki_dir.join("seCall.md");
    std::fs::write(&md_path, "# seCall\n\nstub content for test.\n").expect("write stub md");

    let (status, body) = send_request(&env.router, Method::GET, "/api/wiki/seCall", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["project"], "seCall");
    assert!(
        body["content"]
            .as_str()
            .unwrap_or("")
            .contains("stub content"),
        "content 필드는 stub 본문을 반환해야 함: {body}"
    );
}

// ── 1.6 POST /api/graph ───────────────────────────────────────────────────────

/// 빈 그래프 → 200 + `results: []` + `count: 0`. RestGraphParams DTO 변환
/// (node_id/depth/relation) 도 함께 검증 (잘못 매핑되면 deserialize/handler 에러).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_graph_empty_returns_empty_results() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/graph",
        Some(json!({ "node_id": "project:no-such", "depth": 2 })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["count"], 0);
    assert_eq!(body["query_node"], "project:no-such");
    // depth 는 핸들러에서 min(3) 으로 clamp — 2 그대로 나와야 함.
    assert_eq!(body["depth"], 2);
    assert!(body["results"].is_array());
}

// ── 1.7 POST /api/daily ───────────────────────────────────────────────────────

/// date 미지정 → 오늘 날짜로 처리. 200 + `date` 키 존재.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_daily_default_date_is_today() {
    let env = make_test_env().await;

    let (status, body) =
        send_request(&env.router, Method::POST, "/api/daily", Some(json!({}))).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert_eq!(body["date"], today, "date 미지정 시 오늘로 채워져야 함");
    assert_eq!(body["total_sessions"], 0);
}

/// date 명시 → 그 날짜로 처리. fixture session 1건 + 명시된 날짜.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_daily_with_explicit_date() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-daily-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/daily",
        Some(json!({ "date": "2026-04-01" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["date"], "2026-04-01");
    // 2026-04-01 에는 session 이 없으므로 total_sessions 0.
    assert_eq!(body["total_sessions"], 0);
}

// ── 1.8 GET /api/sessions ────────────────────────────────────────────────────

/// happy path: filter 종합 (project/agent/page/page_size) — 빈 DB → items 빈
/// 배열, total 0.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_sessions_with_filters() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-list-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?project=test-proj&agent=claude-code&page=1&page_size=10",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["id"], "sess-list-1");
}

/// P38 rework — `?tag=` 단일 태그 필터 라우트 회귀.
/// 핵심: 정규화 (대문자 → 소문자) 후 매칭, 미매칭 세션은 제외.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_sessions_filter_by_tag() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-rust");
        insert_minimal_session(&db, "sess-other");
        db.update_session_tags("sess-rust", &["Rust".into()])
            .unwrap();
    }

    let (status, body) =
        send_request(&env.router, Method::GET, "/api/sessions?tag=rust", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(
        body["total"], 1,
        "tag filter must match 1 session, body={body}"
    );
    assert_eq!(body["items"][0]["id"], "sess-rust");
}

/// P38 rework — `?tags=a,b` 다중 태그 AND 필터 라우트 회귀 (P34 Task 03).
/// 핵심: 두 태그 모두 가진 세션만 매칭. axum_extra::Query 의 콤마 split 처리.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_sessions_filter_by_tags_multi_and() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-both");
        insert_minimal_session(&db, "sess-rust-only");
        insert_minimal_session(&db, "sess-search-only");
        db.update_session_tags("sess-both", &["rust".into(), "search".into()])
            .unwrap();
        db.update_session_tags("sess-rust-only", &["rust".into()])
            .unwrap();
        db.update_session_tags("sess-search-only", &["search".into()])
            .unwrap();
    }

    let (status, body) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?tags=rust,search",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(
        body["total"], 1,
        "multi-tag AND must match 1 session, body={body}"
    );
    assert_eq!(body["items"][0]["id"], "sess-both");
}

/// P38 rework — `?favorite=true|false` 즐겨찾기 필터 라우트 회귀.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_sessions_filter_by_favorite() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-fav");
        insert_minimal_session(&db, "sess-normal");
        db.update_session_favorite("sess-fav", true).unwrap();
    }

    let (_status, body_true) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?favorite=true",
        None,
    )
    .await;
    assert_eq!(
        body_true["total"], 1,
        "favorite=true must match 1, body={body_true}"
    );
    assert_eq!(body_true["items"][0]["id"], "sess-fav");

    let (_status, body_false) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?favorite=false",
        None,
    )
    .await;
    assert_eq!(
        body_false["total"], 1,
        "favorite=false must match 1, body={body_false}"
    );
    assert_eq!(body_false["items"][0]["id"], "sess-normal");
}

/// P38 rework — `?date_from=` since 필터 라우트 회귀.
/// `insert_minimal_session` 은 모두 동일 시각이라 since 단독 검증은 매칭 0/N 분기로.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_sessions_filter_by_since() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-recent");
    }

    // 미래 날짜 since → 매칭 0
    let (_status, body_future) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?date_from=2099-01-01",
        None,
    )
    .await;
    assert_eq!(body_future["total"], 0, "future since filter must yield 0");

    // 과거 날짜 since → 매칭 1 (모든 세션)
    let (_status, body_past) = send_request(
        &env.router,
        Method::GET,
        "/api/sessions?date_from=2000-01-01",
        None,
    )
    .await;
    assert_eq!(body_past["total"], 1, "past since filter must yield all");
}

// ── 1.9 GET /api/projects ────────────────────────────────────────────────────

/// 빈 DB → projects 빈 배열.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_projects_empty() {
    let env = make_test_env().await;

    let (status, body) = send_request(&env.router, Method::GET, "/api/projects", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert!(body["projects"].is_array());
    assert_eq!(body["projects"].as_array().unwrap().len(), 0);
}

/// session 추가 후 distinct project 반환.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_projects_with_data() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-proj-1");
    }

    let (_status, body) = send_request(&env.router, Method::GET, "/api/projects", None).await;

    let projects = body["projects"].as_array().expect("projects array");
    assert!(
        projects.iter().any(|p| p == "test-proj"),
        "distinct project 'test-proj' must be present: {body}"
    );
}

// ── 1.10 GET /api/agents ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_agents_empty() {
    let env = make_test_env().await;

    let (status, body) = send_request(&env.router, Method::GET, "/api/agents", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert!(body["agents"].is_array());
    assert_eq!(body["agents"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_agents_with_data() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-agent-1");
    }

    let (_status, body) = send_request(&env.router, Method::GET, "/api/agents", None).await;

    let agents = body["agents"].as_array().expect("agents array");
    assert!(
        !agents.is_empty(),
        "session 1건 추가 후 agents 비어있으면 안 됨: {body}"
    );
}

// ── 1.11 GET /api/tags ───────────────────────────────────────────────────────

/// `with_counts=true` (기본) → tags 가 객체 배열 (`[{name, count}]`).
/// P38 rework — 실제 태그 데이터로 payload shape 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_tags_with_counts_default() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-tag-1");
        insert_minimal_session(&db, "sess-tag-2");
        db.update_session_tags("sess-tag-1", &["rust".into()])
            .unwrap();
        db.update_session_tags("sess-tag-2", &["rust".into()])
            .unwrap();
    }

    let (status, body) = send_request(&env.router, Method::GET, "/api/tags", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    let tags = body["tags"].as_array().expect("tags must be array");
    assert!(
        !tags.is_empty(),
        "tags must not be empty after fixture, body={body}"
    );
    // payload shape: 각 항목이 {name, count} 객체
    let first = &tags[0];
    assert!(
        first.is_object() && first.get("name").is_some() && first.get("count").is_some(),
        "with_counts=true item must be object with name+count, body={body}"
    );
    assert_eq!(first["name"], "rust");
    assert_eq!(first["count"], 2);
}

/// `with_counts=false` → tags 가 문자열 배열.
/// P38 rework — 실제 태그 데이터로 string vs object shape 차이 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_tags_without_counts() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-tag-x");
        db.update_session_tags("sess-tag-x", &["rust".into()])
            .unwrap();
    }

    let (status, body) = send_request(
        &env.router,
        Method::GET,
        "/api/tags?with_counts=false",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    let tags = body["tags"].as_array().expect("tags must be array");
    assert!(
        !tags.is_empty(),
        "tags must not be empty after fixture, body={body}"
    );
    // payload shape: 각 항목이 string (객체 아님)
    let first = &tags[0];
    assert!(
        first.is_string(),
        "with_counts=false item must be plain string (not object), body={body}"
    );
    assert_eq!(first.as_str().unwrap(), "rust");
}

// ─── Section 1A: DTO 변환 회귀 ────────────────────────────────────────────────
//
// `RestRecallParams` / `RestGetParams` / `RestGraphParams` 는 `pub(crate)` 가
// 아닌 모듈-private 타입이므로 외부 통합 테스트에서 `From` impl 을 직접 호출할
// 수 없다 (production code 수정 금지 제약). 대신 라우트를 통해 mode 분기 /
// 필드 rename 매핑이 올바르게 됐음을 응답 형태로 회귀 검증한다.

/// `mode` 미지정 → keyword 로 매핑됨을 라우트 응답으로 간접 검증.
/// (잘못 매핑되면 핸들러가 빈 결과를 반환하지 못하거나 panic 가능)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recall_dto_mode_keyword_default() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/recall",
        Some(json!({ "query": "x" })), // mode 생략 → keyword 기본
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "keyword default must succeed: {body}"
    );
    assert!(body.get("results").is_some());
    assert_eq!(body["count"], 0);
}

/// `mode: "semantic"` → QueryType::Semantic 로 매핑. vector backend 없는
/// 환경에서는 P34 graceful 로 빈 결과 반환 (`{ results: [], count: 0 }`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recall_dto_mode_semantic() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/recall",
        Some(json!({ "query": "x", "mode": "semantic" })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "semantic must gracefully succeed: {body}"
    );
    assert_eq!(body["count"], 0);
}

/// `mode: "temporal"` → QueryType::Temporal 로 매핑. has_keyword=false &&
/// all_results.is_empty() 분기로 즉시 빈 결과 반환.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_recall_dto_mode_temporal() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/recall",
        Some(json!({ "query": "today", "mode": "temporal" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "temporal must succeed: {body}");
    assert_eq!(body["count"], 0);
    assert!(body["results"].is_array());
}

/// `RestGetParams.session_id` → `GetParams.id` 로 rename 매핑됨을
/// /api/get 응답의 `id` 필드로 간접 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_dto_session_id_renamed() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-dto-rename");
    }

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/get",
        Some(json!({ "session_id": "sess-dto-rename" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(
        body["id"], "sess-dto-rename",
        "session_id → GetParams::id 매핑 회귀: {body}"
    );
}

// ─── Section 2: write / commands / jobs 라우트 회귀 (Task 02) ────────────────
//
// 본 섹션은 PATCH 3종 (tags/favorite/notes) + POST commands 4종 + GET/POST jobs
// 4종 + 단일 큐 / cancel 통합 시나리오를 다룬다.
//
// 검증 깊이 원칙:
//   - happy path: status + 핵심 키 1~3개
//   - error path: status + body["error"] 존재
//   - cancel/single-queue: status 전이 polling (50ms × 최대 40회 = 2s timeout)
//
// `spawn_command_job` 의 성공 status 는 `StatusCode::ACCEPTED` (202) 임에 주의
// (rest.rs L476). task 문서의 "200" 표현과 다르지만 production 코드 기준으로 검증.

use std::time::Duration;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

use secall_core::jobs::{BroadcastSink, JobKind};

// ── 2.1 PATCH /api/sessions/{id}/tags ────────────────────────────────────────

/// happy path: 정규화(lowercase) + dedup + sort (BTreeSet 결과).
/// 입력 ["Rust", "DB", "rust"] → 정규화 ["db", "rust"].
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_tags_normalizes_and_dedupes() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-tags-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/sess-tags-1/tags",
        Some(json!({ "tags": ["Rust", "DB", "rust"] })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["session_id"], "sess-tags-1");
    let tags = body["tags"].as_array().expect("tags array");
    let names: Vec<&str> = tags.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(
        names,
        vec!["db", "rust"],
        "정규화 + dedup + 정렬 결과: {body}"
    );
}

/// 미존재 session_id → 500 + error (SessionNotFound → anyhow → error_response).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_tags_missing_session_returns_error() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/no-such/tags",
        Some(json!({ "tags": ["x"] })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing → 500, body={body}"
    );
    assert!(body.get("error").is_some(), "must contain 'error': {body}");
}

// ── 2.2 PATCH /api/sessions/{id}/favorite ────────────────────────────────────

/// happy path: favorite=true 토글 — 응답 echo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_favorite_toggle_true() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-fav-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/sess-fav-1/favorite",
        Some(json!({ "favorite": true })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["session_id"], "sess-fav-1");
    assert_eq!(body["favorite"], true);
}

/// 미존재 session_id → 500 + error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_favorite_missing_session_returns_error() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/no-such/favorite",
        Some(json!({ "favorite": false })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing → 500, body={body}"
    );
    assert!(body.get("error").is_some(), "must contain 'error': {body}");
}

// ── 2.3 PATCH /api/sessions/{id}/notes ───────────────────────────────────────

/// happy path: text notes — 응답 echo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_notes_text_value() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-note-1");
    }

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/sess-note-1/notes",
        Some(json!({ "notes": "hello note" })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["session_id"], "sess-note-1");
    assert_eq!(body["notes"], "hello note");
}

/// `notes: null` (지우기) — 200 + null echo. RestSetNotesBody 가 Option<String>
/// 으로 받음을 검증.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_notes_null_clears() {
    let env = make_test_env().await;
    {
        let db = env.db.lock().unwrap();
        insert_minimal_session(&db, "sess-note-null");
    }

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/sess-note-null/notes",
        Some(json!({ "notes": null })),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["session_id"], "sess-note-null");
    assert!(
        body["notes"].is_null(),
        "null 입력 → null echo 되어야 함: {body}"
    );
}

/// 미존재 session_id → 500 + error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_set_notes_missing_session_returns_error() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::PATCH,
        "/api/sessions/no-such/notes",
        Some(json!({ "notes": "x" })),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "missing → 500, body={body}"
    );
    assert!(body.get("error").is_some(), "must contain 'error': {body}");
}

// ── 2.4 POST /api/commands/{sync,ingest,wiki-update,graph-rebuild} ────────────
//
// `spawn_command_job` 은 성공 시 202 ACCEPTED + `{job_id, status: "started"}`.
// fake adapter 가 즉시 완료하므로 동시성 충돌 없이 4 라우트가 순차 통과.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_command_sync_returns_started() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/sync",
        Some(json!({ "local_only": true, "dry_run": true })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED, "expected 202, body={body}");
    assert!(body["job_id"].is_string(), "job_id must be string: {body}");
    assert_eq!(body["status"], "started");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_command_ingest_returns_started() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/ingest",
        Some(json!({ "force": false, "dry_run": true })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED, "expected 202, body={body}");
    assert!(body["job_id"].is_string());
    assert_eq!(body["status"], "started");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_command_wiki_update_returns_started() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/wiki-update",
        Some(json!({ "dry_run": true })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED, "expected 202, body={body}");
    assert!(body["job_id"].is_string());
    assert_eq!(body["status"], "started");
}

/// P37 graph-rebuild — `retry_failed: true` 분기. body 가 그대로 fake adapter 로
/// 전달돼 outcome echo 에 포함되는지는 별도 (Section 2.5+ 통합 시나리오에서 검증).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_command_graph_rebuild_retry_failed_returns_started() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/graph-rebuild",
        Some(json!({ "retry_failed": true })),
    )
    .await;

    assert_eq!(status, StatusCode::ACCEPTED, "expected 202, body={body}");
    assert!(body["job_id"].is_string());
    assert_eq!(body["status"], "started");
}

// ── 2.5 GET /api/jobs ─────────────────────────────────────────────────────────

/// 빈 active 목록 → 200 + `jobs: []`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_jobs_active_empty() {
    let env = make_test_env().await;

    let (status, body) =
        send_request(&env.router, Method::GET, "/api/jobs?status=active", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    let jobs = body["jobs"].as_array().expect("jobs array");
    assert!(jobs.is_empty(), "no spawn yet → empty: {body}");
}

/// active job 1개 등록 후 list 에 등장. 긴 fake adapter (1s) 로 race 회피.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_list_jobs_active_with_running_job() {
    let env = spawn_long_sync_env(1000).await;
    let job_id = spawn_sync_job(&env, json!({})).await;

    // Started/Running 진입 대기.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let (status, body) =
        send_request(&env.router, Method::GET, "/api/jobs?status=active", None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    let jobs = body["jobs"].as_array().expect("jobs array");
    assert!(
        jobs.iter().any(|j| j["id"] == job_id),
        "active job must appear: {body}"
    );

    // 정리: cancel 후 종료까지 대기 (다른 테스트와 격리는 TestEnv tempdir 가 보장).
    let _ = env.executor.registry.cancel(&job_id).await;
    wait_for_terminal_status(&env, &job_id).await;
}

// ── 2.6 GET /api/jobs/{id} ───────────────────────────────────────────────────

/// 진행 중 job → 200 + JobState (`status: "started"|"running"`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_job_running_returns_state() {
    let env = spawn_long_sync_env(1000).await;
    let job_id = spawn_sync_job(&env, json!({})).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let path = format!("/api/jobs/{job_id}");
    let (status, body) = send_request(&env.router, Method::GET, &path, None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["id"], job_id);
    let st = body["status"].as_str().unwrap_or("");
    assert!(
        matches!(st, "started" | "running"),
        "expected started/running, got {st}: {body}"
    );

    let _ = env.executor.registry.cancel(&job_id).await;
    wait_for_terminal_status(&env, &job_id).await;
}

/// 완료된 job → DB fallback 으로 200 + status: "completed".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_job_completed_returns_db_row() {
    let env = make_test_env().await;
    let job_id = spawn_sync_job(&env, json!({})).await;

    // 즉시 완료 (delay_ms=0).
    let row = wait_for_terminal_status(&env, &job_id).await;
    assert_eq!(row.status, "completed", "expected completed: {row:?}");

    let path = format!("/api/jobs/{job_id}");
    let (status, body) = send_request(&env.router, Method::GET, &path, None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["id"], job_id);
    // DB row 또는 in-memory state — 둘 중 어느 쪽이든 status 는 completed.
    let st = body["status"].as_str().unwrap_or("");
    assert_eq!(st, "completed", "completed status expected: {body}");
}

/// 미존재 job_id → 404 + error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_job_missing_returns_404() {
    let env = make_test_env().await;

    let (status, body) = send_request(&env.router, Method::GET, "/api/jobs/no-such-id", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND, "expected 404, body={body}");
    assert!(body.get("error").is_some(), "must contain 'error': {body}");
}

// ── 2.7 GET /api/jobs/{id}/stream — SSE smoke ────────────────────────────────
//
// `Sse<Stream>` 응답 body 는 stream → axum oneshot 의 to_bytes 로 collect 시
// broadcast Closed 까지 대기. fake adapter 가 즉시 완료 (drop sender) 하면
// stream 도 닫힘. timeout 으로 보호 + body bytes > 0 (initial_state 한 chunk
// 이상) 검증으로 충분 (lagged/disconnect 는 본 phase 외).

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_job_stream_initial_event_smoke() {
    // 짧은 delay (100ms) — fake sync_fn 이 빨리 끝나 broadcast Closed → SSE
    // stream None 으로 닫힘. to_bytes 가 곧 반환됨.
    let env = spawn_long_sync_env(100).await;
    let job_id = spawn_sync_job(&env, json!({})).await;

    // Started 등장 대기 — initial_state 가 active job 의 state 를 담아야 함.
    tokio::time::sleep(Duration::from_millis(20)).await;

    let path = format!("/api/jobs/{job_id}/stream");
    let req = Request::builder()
        .method(Method::GET)
        .uri(&path)
        .body(Body::empty())
        .expect("build request");

    let response = env.router.clone().oneshot(req).await.expect("oneshot ok");

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "SSE must return 200 status"
    );
    let ct = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("text/event-stream"),
        "content-type must be text/event-stream, got {ct}"
    );

    // body 전체 collect 는 안 됨 — registry 가 broadcast sender 를 5분간 보관
    // 하므로 stream 이 자연 종료되지 않는다. 첫 데이터 chunk 만 받아 종료.
    use futures_util::StreamExt;
    let mut stream = response.into_body().into_data_stream();
    let first = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("SSE first chunk must arrive within 2s")
        .expect("stream must yield at least one chunk")
        .expect("chunk must not error");

    assert!(
        !first.is_empty(),
        "SSE first chunk must contain initial_state event payload"
    );
    let s = std::str::from_utf8(&first).expect("utf8 SSE payload");
    assert!(
        s.contains("initial_state"),
        "first SSE event must be initial_state, got: {s}"
    );

    // stream drop 후 정리.
    drop(stream);
    wait_for_terminal_status(&env, &job_id).await;
}

// ── 2.8 POST /api/jobs/{id}/cancel ───────────────────────────────────────────

/// 활성 job cancel → 200 + cancelled:true (idempotent).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cancel_job_active_returns_ok() {
    let env = spawn_long_sync_env(1000).await;
    let job_id = spawn_sync_job(&env, json!({})).await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let path = format!("/api/jobs/{job_id}/cancel");
    let (status, body) = send_request(&env.router, Method::POST, &path, None).await;

    assert_eq!(status, StatusCode::OK, "expected 200, body={body}");
    assert_eq!(body["cancelled"], true);
    assert_eq!(body["job_id"], job_id);

    // 두 번째 cancel — idempotent (registry 가 이미 종료된 job 도 true).
    let (status2, body2) = send_request(&env.router, Method::POST, &path, None).await;
    assert!(
        status2 == StatusCode::OK || status2 == StatusCode::NOT_FOUND,
        "second cancel: 200 (idempotent) 또는 404 (이미 evict) — got {status2}, body={body2}"
    );

    wait_for_terminal_status(&env, &job_id).await;
}

/// 미존재 job_id cancel → 404 + error.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cancel_job_missing_returns_404() {
    let env = make_test_env().await;

    let (status, body) = send_request(
        &env.router,
        Method::POST,
        "/api/jobs/no-such-id/cancel",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "expected 404, body={body}");
    assert!(body.get("error").is_some(), "must contain 'error': {body}");
}

// ── 2.9 통합 시나리오: 단일 큐 (P33) ─────────────────────────────────────────

/// P33 단일 큐 정책: 활성 job 있을 때 두 번째 spawn 시도 → 409 + error.
/// graph-rebuild 도 같은 spawn_gate 를 공유하므로 sync 진행 중이면 거절.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_commands_single_queue_returns_409() {
    // 첫 sync 가 충분히 길게 살아있도록 1s delay.
    let env = spawn_long_sync_env(1000).await;

    // 첫 spawn: sync.
    let (status1, body1) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/sync",
        Some(json!({ "local_only": true })),
    )
    .await;
    assert_eq!(status1, StatusCode::ACCEPTED, "first must succeed: {body1}");
    let job_id = body1["job_id"].as_str().expect("job_id").to_string();

    // Started/Running 진입 대기 (try_spawn 직후 상태 등록 race 방지).
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 두 번째 spawn: graph-rebuild → 409 단일 큐 거절.
    let (status2, body2) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/graph-rebuild",
        Some(json!({ "retry_failed": true })),
    )
    .await;
    assert_eq!(
        status2,
        StatusCode::CONFLICT,
        "second must be 409 single-queue: {body2}"
    );
    assert!(
        body2.get("error").is_some(),
        "409 body must contain 'error': {body2}"
    );

    // 정리.
    let _ = env.executor.registry.cancel(&job_id).await;
    wait_for_terminal_status(&env, &job_id).await;
}

// ── 2.10 통합 시나리오: cancel → interrupted (P36) ───────────────────────────

/// 긴 graph-rebuild spawn → cancel POST → GET /api/jobs/{id} status=interrupted.
/// fake graph_rebuild_fn 이 50ms 슬라이스로 is_cancelled() 폴링 → 부분 outcome
/// 반환 → executor 가 interrupted 로 finalize.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_command_then_cancel_yields_interrupted() {
    let env = spawn_long_graph_env(2000).await;

    let (status1, body1) = send_request(
        &env.router,
        Method::POST,
        "/api/commands/graph-rebuild",
        Some(json!({ "all": true })),
    )
    .await;
    assert_eq!(status1, StatusCode::ACCEPTED, "spawn must succeed: {body1}");
    let job_id = body1["job_id"].as_str().expect("job_id").to_string();

    // Running 진입 대기 (첫 슬라이스 진입 보장).
    tokio::time::sleep(Duration::from_millis(150)).await;

    // POST cancel.
    let cancel_path = format!("/api/jobs/{job_id}/cancel");
    let (cstatus, cbody) = send_request(&env.router, Method::POST, &cancel_path, None).await;
    assert_eq!(cstatus, StatusCode::OK, "cancel must return 200: {cbody}");
    assert_eq!(cbody["cancelled"], true);

    // status=interrupted 까지 폴링 (최대 ~3s).
    let mut final_row = None;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = env
            .executor
            .db
            .lock()
            .unwrap()
            .get_job(&job_id)
            .expect("get_job ok");
        if let Some(r) = row {
            if r.status == "interrupted" || r.status == "completed" || r.status == "failed" {
                final_row = Some(r);
                break;
            }
        }
    }
    let row = final_row.expect("job did not finalize within 3s");
    assert_eq!(
        row.status, "interrupted",
        "cancel must yield interrupted: {row:?}"
    );
}

// ─── 헬퍼 (Section 2 전용) ────────────────────────────────────────────────────
//
// `common::make_test_env` 는 delay_ms=0 fake adapter 를 주입한다. 본 섹션의
// race/cancel 시나리오는 충분히 긴 delay 가 필요하므로 별도 builder 를 둔다.
// `common/mod.rs` 는 Task 00 영역이라 helper 만 본 파일에 둠.

use secall_core::store::JobRow;

async fn spawn_long_sync_env(delay_ms: u64) -> common::TestEnv {
    use std::sync::Arc;
    use std::sync::Mutex;

    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.db");
    let db = secall_core::store::Database::open(&db_path).expect("open db");
    let db_arc = Arc::new(Mutex::new(db));

    let executor = Arc::new(secall_core::jobs::JobExecutor::with_adapters(
        db_arc.clone(),
        common::make_fake_adapters(delay_ms),
    ));

    let tok = secall_core::search::LinderaKoTokenizer::new().expect("tokenizer init");
    let engine = secall_core::search::SearchEngine::new(
        secall_core::search::Bm25Indexer::new(Box::new(tok)),
        None,
    );
    let vault_path = dir.path().join("vault");
    let server =
        secall_core::mcp::SeCallMcpServer::new(db_arc.clone(), Arc::new(engine), vault_path);
    let router = secall_core::mcp::rest::rest_router(server, executor.clone());

    common::TestEnv {
        _tempdir: dir,
        db: db_arc,
        executor,
        router,
    }
}

/// graph_rebuild_fn 의 cancel 폴링 (50ms slice) 이 동작하려면 delay_ms 가
/// 충분해야 한다. spawn_long_sync_env 와 동일 구조 — 별도 alias 로만 분리.
async fn spawn_long_graph_env(delay_ms: u64) -> common::TestEnv {
    spawn_long_sync_env(delay_ms).await
}

/// `/api/commands/sync` POST 응답에서 job_id 만 추출.
async fn spawn_sync_job(env: &common::TestEnv, body: serde_json::Value) -> String {
    let (status, resp) =
        send_request(&env.router, Method::POST, "/api/commands/sync", Some(body)).await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "sync spawn must succeed: {resp}"
    );
    resp["job_id"].as_str().expect("job_id missing").to_string()
}

/// DB row 가 terminal status (completed/failed/interrupted) 가 될 때까지 폴링.
/// 최대 ~2s. 테스트 정리에 사용.
async fn wait_for_terminal_status(env: &common::TestEnv, job_id: &str) -> JobRow {
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let row = env
            .executor
            .db
            .lock()
            .unwrap()
            .get_job(job_id)
            .expect("get_job ok");
        if let Some(r) = row {
            if r.status == "completed" || r.status == "failed" || r.status == "interrupted" {
                return r;
            }
        }
    }
    panic!("job {job_id} did not finalize within 2s");
}

/// 현재 사용 안 함이지만 dead_code 경고 방지를 위해 명시적 사용 — JobKind/BroadcastSink
/// 는 helper 미래 확장용.
#[allow(dead_code)]
fn _typecheck_imports() {
    let _ = JobKind::Sync;
    let _: Option<BroadcastSink> = None;
}
