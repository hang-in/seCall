use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};

use super::server::SeCallMcpServer;
use super::tools::{
    GetParams, GraphQueryParams, QueryItem, QueryType, RecallParams, WikiSearchParams,
};
use crate::search::hybrid::SearchEngine;
use crate::store::db::Database;

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

type AppState = Arc<SeCallMcpServer>;

/// REST API 라우터 생성
pub fn rest_router(server: SeCallMcpServer) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state: AppState = Arc::new(server);

    Router::new()
        .route("/api/recall", post(api_recall))
        .route("/api/get", post(api_get))
        .route("/api/status", get(api_status))
        .route("/api/wiki", post(api_wiki))
        .route("/api/graph", post(api_graph))
        .layer(cors)
        .with_state(state)
}

/// REST + MCP 통합 서버 시작 (loopback 전용)
pub async fn start_rest_server(
    db: Database,
    search: SearchEngine,
    vault_path: std::path::PathBuf,
    port: u16,
) -> anyhow::Result<()> {
    let db_arc = Arc::new(std::sync::Mutex::new(db));
    let search_arc = Arc::new(search);
    let server = SeCallMcpServer::new(db_arc, search_arc, vault_path);
    let router = rest_router(server);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(addr = %addr, "REST API server listening");
    tracing::info!("endpoints: /api/recall, /api/get, /api/status, /api/wiki, /api/graph");

    axum::serve(listener, router).await?;
    Ok(())
}

async fn api_recall(
    State(s): State<AppState>,
    Json(p): Json<RestRecallParams>,
) -> impl IntoResponse {
    match s.do_recall(p.into()).await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_get(
    State(s): State<AppState>,
    Json(p): Json<RestGetParams>,
) -> impl IntoResponse {
    match s.do_get(p.into()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_status(State(s): State<AppState>) -> impl IntoResponse {
    match s.do_status() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_wiki(
    State(s): State<AppState>,
    Json(p): Json<WikiSearchParams>,
) -> impl IntoResponse {
    match s.do_wiki_search(p) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_graph(
    State(s): State<AppState>,
    Json(p): Json<GraphQueryParams>,
) -> impl IntoResponse {
    match s.do_graph_query(p) {
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
