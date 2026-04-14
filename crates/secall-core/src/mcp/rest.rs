use std::sync::Arc;

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::server::SeCallMcpServer;
use super::tools::{GetParams, GraphQueryParams, RecallParams, WikiSearchParams};
use crate::search::hybrid::SearchEngine;
use crate::store::db::Database;

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
    Json(p): Json<RecallParams>,
) -> impl IntoResponse {
    match s.do_recall(p).await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_get(
    State(s): State<AppState>,
    Json(p): Json<GetParams>,
) -> impl IntoResponse {
    match s.do_get(p) {
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
