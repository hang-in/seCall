use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{FromRef, Json, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, patch, post},
    Router,
};
use futures_util::stream::Stream;
use serde::Deserialize;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use super::server::SeCallMcpServer;
use super::tools::{
    GetParams, GraphQueryParams, QueryItem, QueryType, RecallParams, WikiSearchParams,
};
use crate::jobs::{BroadcastSink, JobExecutor, JobKind, ProgressEvent};
use crate::search::hybrid::SearchEngine;
use crate::store::db::Database;
use crate::store::session_repo::SessionListFilter;

// ── REST 간소화 DTO ─────────────────────────────────────────
// MCP 스키마를 직접 노출하지 않고 REST 클라이언트에 친화적인 형태로 받아서 변환

#[derive(Debug, Deserialize)]
struct RestRecallParams {
    query: String,
    #[serde(default)]
    mode: Option<String>, // "keyword" | "semantic" — 기본 keyword
    project: Option<String>,
    agent: Option<String>,
    limit: Option<usize>,
}

impl From<RestRecallParams> for RecallParams {
    fn from(p: RestRecallParams) -> Self {
        let query_type = match p.mode.as_deref() {
            Some("semantic") => QueryType::Semantic,
            Some("temporal") => QueryType::Temporal,
            _ => QueryType::Keyword,
        };
        RecallParams {
            queries: vec![QueryItem {
                query_type,
                query: p.query,
            }],
            project: p.project,
            agent: p.agent,
            limit: p.limit,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RestGetParams {
    session_id: String,
    #[serde(default)]
    full: Option<bool>,
}

impl From<RestGetParams> for GetParams {
    fn from(p: RestGetParams) -> Self {
        GetParams {
            id: p.session_id,
            full: p.full,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RestDailyParams {
    date: Option<String>, // "YYYY-MM-DD", 기본 오늘
}

#[derive(Debug, Deserialize)]
struct RestGraphParams {
    node_id: String,
    depth: Option<usize>,
    relation: Option<String>,
}

impl From<RestGraphParams> for GraphQueryParams {
    fn from(p: RestGraphParams) -> Self {
        GraphQueryParams {
            node_id: p.node_id,
            depth: p.depth,
            relation: p.relation,
        }
    }
}

/// REST API의 공유 상태.
///
/// `FromRef`로 기존 핸들러가 `State<Arc<SeCallMcpServer>>`를 그대로 받을 수 있게
/// 하고, Job 핸들러는 `State<Arc<JobExecutor>>`를 받는다. 새 필드 추가 시 둘 다
/// `FromRef` 구현을 추가하면 핸들러 시그니처에 영향 없음.
#[derive(Clone)]
pub struct AppState {
    pub server: Arc<SeCallMcpServer>,
    pub executor: Arc<JobExecutor>,
}

impl FromRef<AppState> for Arc<SeCallMcpServer> {
    fn from_ref(state: &AppState) -> Self {
        state.server.clone()
    }
}

impl FromRef<AppState> for Arc<JobExecutor> {
    fn from_ref(state: &AppState) -> Self {
        state.executor.clone()
    }
}

/// REST API 라우터 생성
pub fn rest_router(server: SeCallMcpServer, executor: Arc<JobExecutor>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state = AppState {
        server: Arc::new(server),
        executor,
    };

    let api = Router::new()
        .route("/api/recall", post(api_recall))
        .route("/api/get", post(api_get))
        .route("/api/status", get(api_status))
        .route("/api/wiki", post(api_wiki))
        .route("/api/wiki/{project}", get(api_wiki_get))
        .route("/api/graph", post(api_graph))
        .route("/api/daily", post(api_daily))
        .route("/api/sessions", get(api_list_sessions))
        .route("/api/projects", get(api_list_projects))
        .route("/api/agents", get(api_list_agents))
        .route("/api/sessions/{id}/tags", patch(api_set_tags))
        .route("/api/sessions/{id}/favorite", patch(api_set_favorite))
        .route("/api/sessions/{id}/notes", patch(api_set_notes))
        // P33 Task 03 — Job 시스템
        .route("/api/commands/sync", post(api_command_sync))
        .route("/api/commands/ingest", post(api_command_ingest))
        .route("/api/commands/wiki-update", post(api_command_wiki_update))
        .route("/api/jobs", get(api_list_jobs))
        .route("/api/jobs/{id}", get(api_get_job))
        .route("/api/jobs/{id}/stream", get(api_job_stream))
        .route("/api/jobs/{id}/cancel", post(api_cancel_job))
        .layer(cors)
        .with_state(state);

    // web router는 fallback으로 (api 경로가 우선 매칭됨)
    #[cfg(feature = "web-ui")]
    let api = api.merge(crate::web::web_router());

    api
}

/// REST + MCP 통합 서버 시작 (loopback 전용)
pub async fn start_rest_server(
    db_arc: Arc<std::sync::Mutex<Database>>,
    search: SearchEngine,
    vault_path: std::path::PathBuf,
    port: u16,
    executor: Arc<JobExecutor>,
) -> anyhow::Result<()> {
    let search_arc = Arc::new(search);
    let server = SeCallMcpServer::new(db_arc, search_arc, vault_path);
    let router = rest_router(server, executor);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(addr = %addr, "REST API server listening");
    tracing::info!(
        "endpoints: /api/recall, /api/get, /api/status, /api/wiki, /api/graph, /api/daily, \
         /api/commands/{{sync,ingest,wiki-update}}, /api/jobs, /api/jobs/:id, \
         /api/jobs/:id/stream, /api/jobs/:id/cancel"
    );

    axum::serve(listener, router).await?;
    Ok(())
}

async fn api_recall(
    State(s): State<Arc<SeCallMcpServer>>,
    Json(p): Json<RestRecallParams>,
) -> impl IntoResponse {
    match s.do_recall(p.into()).await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_get(
    State(s): State<Arc<SeCallMcpServer>>,
    Json(p): Json<RestGetParams>,
) -> impl IntoResponse {
    match s.do_get(p.into()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_status(State(s): State<Arc<SeCallMcpServer>>) -> impl IntoResponse {
    match s.do_status() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_wiki(
    State(s): State<Arc<SeCallMcpServer>>,
    Json(p): Json<WikiSearchParams>,
) -> impl IntoResponse {
    match s.do_wiki_search(p) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_wiki_get(
    State(s): State<Arc<SeCallMcpServer>>,
    AxumPath(project): AxumPath<String>,
) -> impl IntoResponse {
    match s.do_wiki_get(&project) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"error": msg})),
                )
                    .into_response()
            } else {
                error_response(e)
            }
        }
    }
}

async fn api_graph(
    State(s): State<Arc<SeCallMcpServer>>,
    Json(p): Json<RestGraphParams>,
) -> impl IntoResponse {
    match s.do_graph_query(p.into()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_daily(
    State(s): State<Arc<SeCallMcpServer>>,
    Json(p): Json<RestDailyParams>,
) -> impl IntoResponse {
    let date = p
        .date
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    match s.do_daily(&date) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

fn error_response(e: anyhow::Error) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    )
        .into_response()
}

// ─── REST listing / mutation (P32 Task 02) ─────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct SessionListQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    project: Option<String>,
    agent: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    tag: Option<String>,
    /// P34 Task 03: 다중 태그 AND 매칭. 두 형태 모두 지원:
    ///   - 반복 파라미터: `?tags=rust&tags=search` → ["rust", "search"]
    ///   - 콤마 구분:    `?tags=rust,search`       → ["rust,search"] (아래 split로 분해)
    ///   - 혼합:        `?tags=rust,db&tags=search` → 모두 split 후 합침
    ///
    /// 반복 파라미터 수용을 위해 `axum_extra::extract::Query`(serde_html_form)를 사용한다.
    tags: Option<Vec<String>>,
    favorite: Option<bool>,
    q: Option<String>,
}

impl From<SessionListQuery> for SessionListFilter {
    fn from(q: SessionListQuery) -> Self {
        let tags: Vec<String> = q
            .tags
            .unwrap_or_default()
            .into_iter()
            .flat_map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .filter(|s| !s.is_empty())
            .collect();
        SessionListFilter {
            project: q.project,
            agent: q.agent,
            date_from: q.date_from,
            date_to: q.date_to,
            tag: q.tag,
            tags,
            favorite: q.favorite,
            q: q.q,
            page: q.page.unwrap_or(1),
            page_size: q.page_size.unwrap_or(30),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SetTagsBody {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SetFavoriteBody {
    favorite: bool,
}

#[derive(Debug, Deserialize)]
struct SetNotesBody {
    /// `Option`으로 받아 `null` 또는 빈 문자열 모두 허용. 사용자 의도 보존.
    notes: Option<String>,
}

async fn api_list_sessions(
    State(s): State<Arc<SeCallMcpServer>>,
    // axum_extra::extract::Query는 동일 키 반복(`?tags=a&tags=b`)을 Vec로 수용한다.
    // 기본 axum::extract::Query (serde_urlencoded)는 마지막 값만 남기므로 여기만 분기.
    axum_extra::extract::Query(q): axum_extra::extract::Query<SessionListQuery>,
) -> impl IntoResponse {
    match s.do_list_sessions(q.into()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_list_projects(State(s): State<Arc<SeCallMcpServer>>) -> impl IntoResponse {
    match s.do_list_projects() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_list_agents(State(s): State<Arc<SeCallMcpServer>>) -> impl IntoResponse {
    match s.do_list_agents() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_set_tags(
    State(s): State<Arc<SeCallMcpServer>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetTagsBody>,
) -> impl IntoResponse {
    match s.do_set_tags(&id, body.tags) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_set_favorite(
    State(s): State<Arc<SeCallMcpServer>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetFavoriteBody>,
) -> impl IntoResponse {
    match s.do_set_favorite(&id, body.favorite) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_set_notes(
    State(s): State<Arc<SeCallMcpServer>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetNotesBody>,
) -> impl IntoResponse {
    match s.do_set_notes(&id, body.notes.as_deref()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

// ─── P33 Task 03 — Job 시스템 핸들러 ─────────────────────────────────────────

/// 단일 mutating job 시작 공통 헬퍼.
///
/// `args_value`는 어댑터가 받는 JSON erased args. Adapter가 None이면 500.
/// Active job이 있으면 409 + `current_kind` 안내.
async fn spawn_command_job(
    executor: Arc<JobExecutor>,
    kind: JobKind,
    args_value: serde_json::Value,
) -> axum::response::Response {
    let adapters = match executor.adapters.clone() {
        Some(a) => a,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "command adapters not configured on this executor",
                })),
            )
                .into_response();
        }
    };

    let metadata = Some(args_value.clone());

    let spawn_result = executor
        .try_spawn(kind, metadata, move |tx| {
            let adapters = adapters.clone();
            let args_value = args_value.clone();
            async move {
                let sink = BroadcastSink::new(tx);
                let fut = match kind {
                    JobKind::Sync => (adapters.sync_fn)(args_value, sink),
                    JobKind::Ingest => (adapters.ingest_fn)(args_value, sink),
                    JobKind::WikiUpdate => (adapters.wiki_update_fn)(args_value, sink),
                };
                fut.await
            }
        })
        .await;

    match spawn_result {
        Some((id, _tx)) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "job_id": id,
                "status": "started",
            })),
        )
            .into_response(),
        None => {
            let current = executor
                .registry
                .current_active_kind()
                .await
                .map(|k| k.as_str());
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "another mutating job is running",
                    "current_kind": current,
                })),
            )
                .into_response()
        }
    }
}

async fn api_command_sync(
    State(executor): State<Arc<JobExecutor>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    spawn_command_job(executor, JobKind::Sync, body).await
}

async fn api_command_ingest(
    State(executor): State<Arc<JobExecutor>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    spawn_command_job(executor, JobKind::Ingest, body).await
}

async fn api_command_wiki_update(
    State(executor): State<Arc<JobExecutor>>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    spawn_command_job(executor, JobKind::WikiUpdate, body).await
}

#[derive(Debug, Deserialize, Default)]
struct ListJobsQuery {
    /// "active" (default) — 메모리 in-flight, "recent" — DB 최근 50개
    status: Option<String>,
    limit: Option<usize>,
}

async fn api_list_jobs(
    State(executor): State<Arc<JobExecutor>>,
    Query(q): Query<ListJobsQuery>,
) -> impl IntoResponse {
    match q.status.as_deref() {
        Some("recent") => {
            let limit = q.limit.unwrap_or(50).clamp(1, 200);
            let rows = match executor.db.lock() {
                Ok(db) => match db.list_recent_jobs(limit) {
                    Ok(rows) => rows,
                    Err(e) => return error_response(anyhow::anyhow!(e)),
                },
                Err(_) => {
                    return error_response(anyhow::anyhow!("db lock poisoned"));
                }
            };
            (StatusCode::OK, Json(serde_json::json!({ "jobs": rows }))).into_response()
        }
        // 기본 또는 "active"
        _ => {
            let states = executor.registry.list_active().await;
            (StatusCode::OK, Json(serde_json::json!({ "jobs": states }))).into_response()
        }
    }
}

async fn api_get_job(
    State(executor): State<Arc<JobExecutor>>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    if let Some(state) = executor.registry.get(&id).await {
        return (StatusCode::OK, Json(serde_json::to_value(state).unwrap())).into_response();
    }
    let row = match executor.db.lock() {
        Ok(db) => match db.get_job(&id) {
            Ok(opt) => opt,
            Err(e) => return error_response(anyhow::anyhow!(e)),
        },
        Err(_) => return error_response(anyhow::anyhow!("db lock poisoned")),
    };
    match row {
        Some(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "job not found" })),
        )
            .into_response(),
    }
}

/// `/api/jobs/:id/stream` — SSE 또는 단발 JSON.
///
/// - 메모리에 있으면 broadcast 구독 + 첫 이벤트로 현재 `JobState`를 push (재접속 시 phase 복원)
/// - 메모리에 없고 DB에만 있으면 단발 JSON 응답 (이미 완료된 job)
/// - 둘 다 없으면 404
async fn api_job_stream(
    State(executor): State<Arc<JobExecutor>>,
    AxumPath(id): AxumPath<String>,
) -> axum::response::Response {
    // 메모리 우선
    if let Some(initial_state) = executor.registry.get(&id).await {
        let receiver = match executor.registry.subscribe(&id).await {
            Some(r) => r,
            None => {
                return (
                    StatusCode::GONE,
                    Json(serde_json::json!({ "error": "job already evicted" })),
                )
                    .into_response();
            }
        };
        let stream = job_event_stream(initial_state, receiver);
        return Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response();
    }

    // DB fallback (이미 완료된 job)
    let row = match executor.db.lock() {
        Ok(db) => match db.get_job(&id) {
            Ok(opt) => opt,
            Err(e) => return error_response(anyhow::anyhow!(e)),
        },
        Err(_) => return error_response(anyhow::anyhow!("db lock poisoned")),
    };
    match row {
        Some(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "job not found" })),
        )
            .into_response(),
    }
}

/// SSE 이벤트 스트림: 첫 이벤트로 initial state, 그 다음 broadcast recv 결과를 push.
///
/// `Lagged`는 무시하고 다음 이벤트로 진행 (구독자가 늦어도 스트림 자체는 닫지 않음).
/// `Closed`(broadcast가 dropped)면 스트림 종료.
fn job_event_stream(
    initial: crate::jobs::JobState,
    receiver: broadcast::Receiver<ProgressEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    use tokio::sync::broadcast::error::RecvError;

    enum Phase {
        Initial(Box<crate::jobs::JobState>),
        Streaming,
    }

    futures_util::stream::unfold(
        (Phase::Initial(Box::new(initial)), receiver),
        |(phase, mut rx)| async move {
            match phase {
                Phase::Initial(state) => {
                    let json = serde_json::json!({
                        "type": "initial_state",
                        "state": state,
                    })
                    .to_string();
                    Some((
                        Ok::<Event, Infallible>(Event::default().data(json)),
                        (Phase::Streaming, rx),
                    ))
                }
                Phase::Streaming => loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            return Some((
                                Ok::<Event, Infallible>(Event::default().data(json)),
                                (Phase::Streaming, rx),
                            ));
                        }
                        Err(RecvError::Lagged(_)) => {
                            // 다음 이벤트 시도 (Lagged는 단순 누락 통지)
                            continue;
                        }
                        Err(RecvError::Closed) => return None,
                    }
                },
            }
        },
    )
}

async fn api_cancel_job(
    State(_executor): State<Arc<JobExecutor>>,
    AxumPath(_id): AxumPath<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({
            "error": "cancellation not supported in P33 MVP — planned for v1.1",
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    //! P34 Task 03 rework — `?tags=` 쿼리 파라미터 파싱 회귀 방지.
    //!
    //! 1차 구현은 `axum::extract::Query` + `Option<String>`이라 `?tags=a&tags=b`
    //! 형태(반복 키)에서 마지막 값만 살아남았다. 본 fix는 axum-extra의 Query
    //! (serde_html_form 기반) + `Option<Vec<String>>` + 콤마 split 조합으로
    //! 반복/콤마/혼합 세 경우를 모두 수용한다.
    use super::*;
    use axum::extract::FromRequestParts;
    use axum::http::Request;
    use axum_extra::extract::Query as ExtraQuery;

    async fn parse_query(uri: &str) -> SessionListQuery {
        let req = Request::builder().uri(uri).body(()).unwrap();
        let (mut parts, _) = req.into_parts();
        let ExtraQuery(q): ExtraQuery<SessionListQuery> =
            ExtraQuery::from_request_parts(&mut parts, &())
                .await
                .expect("query must parse");
        q
    }

    #[tokio::test]
    async fn list_query_parses_repeated_tags() {
        let q = parse_query("/?tags=rust&tags=search").await;
        assert_eq!(q.tags, Some(vec!["rust".to_string(), "search".to_string()]));
        let f: SessionListFilter = q.into();
        assert_eq!(f.tags, vec!["rust".to_string(), "search".to_string()]);
    }

    #[tokio::test]
    async fn list_query_parses_comma_delimited_tags() {
        let q = parse_query("/?tags=rust,search").await;
        let f: SessionListFilter = q.into();
        assert_eq!(f.tags, vec!["rust".to_string(), "search".to_string()]);
    }

    #[tokio::test]
    async fn list_query_parses_mixed_repeat_and_comma_tags() {
        let q = parse_query("/?tags=rust,db&tags=search").await;
        let f: SessionListFilter = q.into();
        assert_eq!(
            f.tags,
            vec!["rust".to_string(), "db".to_string(), "search".to_string()]
        );
    }

    #[tokio::test]
    async fn list_query_drops_empty_and_whitespace_tag_entries() {
        let q = parse_query("/?tags=,rust,&tags=%20%20").await;
        let f: SessionListFilter = q.into();
        assert_eq!(f.tags, vec!["rust".to_string()]);
    }
}
