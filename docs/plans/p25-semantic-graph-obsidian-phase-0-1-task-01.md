---
type: task
status: ready
plan: p25-semantic-graph-obsidian-phase-0-1
task_number: 1
title: REST API 서버 (secall serve)
updated_at: 2026-04-14
---

# Task 01 — REST API 서버 (`secall serve`)

## 목표

MCP tool 로직을 `do_*()` 공통 메서드로 추출하고, REST 엔드포인트를 추가하여
`secall serve --port 8080`으로 REST + MCP를 동시에 서빙한다.

## Changed files

### 1. `crates/secall-core/src/mcp/server.rs` (수정)

**대상**: `SeCallMcpServer` impl 블록 (line 42~492)

5개 MCP tool 각각에서 핵심 로직을 `do_*` pub 메서드로 추출.

**추출 대상 메서드**:

| 현재 tool 메서드 | 추출할 pub 메서드 | 반환 타입 |
|-----------------|------------------|-----------|
| `recall()` (line 57) | `pub async fn do_recall(&self, params: RecallParams) -> anyhow::Result<serde_json::Value>` | JSON |
| `get()` (line 190) | `pub fn do_get(&self, params: GetParams) -> anyhow::Result<serde_json::Value>` | JSON |
| `status()` (line 245) | `pub fn do_status(&self) -> anyhow::Result<serde_json::Value>` | JSON |
| `wiki_search()` (line 266) | `pub fn do_wiki_search(&self, params: WikiSearchParams) -> anyhow::Result<serde_json::Value>` | JSON |
| `graph_query()` (line 404) | `pub fn do_graph_query(&self, params: GraphQueryParams) -> anyhow::Result<serde_json::Value>` | JSON |

**반환 타입 근거**: `SecallError`에 DB lock 실패 variant가 없으므로 (error.rs line 1-48) `anyhow::Result`로 통일.

**변환 패턴** (recall 예시):
```rust
// 추출된 공통 메서드
pub async fn do_recall(&self, params: RecallParams) -> anyhow::Result<serde_json::Value> {
    let limit = params.limit.unwrap_or(10).min(50);
    // ... 기존 recall() 로직 그대로 ...
    // McpError → anyhow::anyhow!() 로 변환
    // 마지막: CallToolResult 래핑 대신 json!({...}) 직접 반환
    Ok(json!({ "results": all_results, "count": count, "related_sessions": related_sessions }))
}

// MCP tool wrapper (기존 시그니처 유지)
#[tool(...)]
async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> Result<CallToolResult, McpError> {
    let json = self.do_recall(params).await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )]))
}
```

**status 특별 처리**: 현재 `String` 반환 → `do_status()`는 `serde_json::Value` 반환하도록 변경.
MCP tool wrapper에서는 `Content::text(to_string_pretty(...))` 로 래핑.

```rust
pub fn do_status(&self) -> anyhow::Result<serde_json::Value> {
    let db = self.db.lock().map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
    let stats = db.get_stats()?;  // DbStats: line 212
    Ok(serde_json::json!({
        "sessions": stats.session_count,
        "turns": stats.turn_count,
        "vectors": stats.vector_count,
        "recent_ingests": stats.recent_ingests.len(),
    }))
}
```

### 2. `crates/secall-core/src/mcp/rest.rs` (신규)

REST 핸들러 + axum Router + CORS 설정.

```rust
use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tower_http::cors::{CorsLayer, Any};
use super::server::SeCallMcpServer;
use super::tools::{RecallParams, GetParams, WikiSearchParams, GraphQueryParams};

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

/// REST + MCP 통합 서버 시작
pub async fn start_rest_server(
    db: crate::store::Database,
    search: crate::search::SearchEngine,
    vault_path: std::path::PathBuf,
    port: u16,
) -> anyhow::Result<()> {
    // SeCallMcpServer 인스턴스 생성
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
```

**핸들러 패턴** (5개 모두 동일):
```rust
async fn api_recall(State(s): State<AppState>, Json(p): Json<RecallParams>) -> impl IntoResponse {
    match s.do_recall(p).await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

async fn api_status(State(s): State<AppState>) -> impl IntoResponse {
    match s.do_status() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}
```

### 3. `crates/secall-core/src/mcp/mod.rs` (수정)

현재 (line 1-5):
```rust
pub mod instructions;
pub mod server;
pub mod tools;

pub use server::{start_mcp_http_server, start_mcp_server, SeCallMcpServer};
```

추가:
```rust
pub mod rest;
pub use rest::start_rest_server;
```

### 4. `Cargo.toml` — 워크스페이스 (수정)

`[workspace.dependencies]` 섹션에 tower-http 추가:
```toml
tower-http = { version = "0.6", features = ["cors"] }
```

### 5. `crates/secall-core/Cargo.toml` (수정)

`[dependencies]` 섹션에 추가:
```toml
tower-http.workspace = true
```

### 6. `crates/secall/src/commands/serve.rs` (신규)

```rust
use anyhow::Result;
use secall_core::{
    mcp::rest::start_rest_server,
    search::tokenizer::create_tokenizer,
    search::vector::create_vector_indexer,
    search::{Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub async fn run(port: u16) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow::anyhow!("tokenizer init failed: {e}"))?;
    let bm25 = Bm25Indexer::new(tok);
    let vector = create_vector_indexer(&config).await;
    let search = SearchEngine::new(bm25, vector);
    let vault_path = config.vault.path.clone();

    start_rest_server(db, search, vault_path, port).await
}
```

### 7. `crates/secall/src/commands/mod.rs` (수정)

추가:
```rust
pub mod serve;
```

### 8. `crates/secall/src/main.rs` (수정)

`Commands` enum (line 21~226)에 `Serve` variant 추가:
```rust
/// Start REST + MCP API server
Serve {
    /// Port number (default: 8080)
    #[arg(long, short, default_value = "8080")]
    port: u16,
},
```

`match cli.command` (line 346~)에 arm 추가:
```rust
Commands::Serve { port } => {
    commands::serve::run(port).await?;
}
```

## Change description

1. `server.rs`에서 5개 MCP tool의 핵심 로직을 `do_*()` pub 메서드로 추출
2. 기존 `#[tool]` 메서드는 `do_*()` 호출 → CallToolResult 래핑하는 thin wrapper로 변경
3. `rest.rs` 신규 생성: axum REST 핸들러 5개 + CorsLayer + `start_rest_server()`
4. CLI에 `secall serve --port` 명령 추가
5. tower-http 의존성 추가 (CORS용)

## Dependencies

- 다른 subtask 의존성 없음 (첫 번째 Task)
- 추가 패키지: `tower-http 0.6` (axum 0.8 호환)

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -5

# 2. 기존 테스트 통과 확인 (MCP 리팩터링이 기존 동작을 깨지 않는지)
cargo test 2>&1 | tail -10

# 3. serve 명령 help 확인
cargo run -- serve --help

# 4. REST API 통합 테스트 (Manual)
# 터미널 1: cargo run -- serve --port 8080
# 터미널 2:
#   curl -s http://127.0.0.1:8080/api/status
#   curl -s -X POST http://127.0.0.1:8080/api/recall \
#     -H 'Content-Type: application/json' \
#     -d '{"queries":[{"type":"keyword","query":"rust"}]}'
#   curl -s -X POST http://127.0.0.1:8080/api/get \
#     -H 'Content-Type: application/json' \
#     -d '{"id":"6e5dd35d"}'
```

## Risks

- **`#[tool_router]` 매크로 간섭**: `do_*` 메서드에는 `#[tool]` 어트리뷰트가 없으므로 매크로가 무시할 것으로 예상. 만약 간섭하면 `do_*` 메서드를 별도 `impl` 블록으로 분리.
- **tower-http 버전**: axum 0.8 → tower-http 0.6.x가 호환 범위. `cargo check`에서 버전 충돌 시 0.5로 하향.
- **DB lock contention**: REST와 MCP가 같은 `Arc<Mutex<Database>>`를 공유. 현재 MCP도 동일 패턴이므로 추가 위험 없음.
- **`SeCallMcpServer` Clone**: 이미 `#[derive(Clone)]` — `Arc::new(server)` 후 핸들러에서 공유 가능.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/mcp/tools.rs` — param struct 정의는 변경하지 않음 (Serialize 추가 시에만 허용)
- `crates/secall-core/src/mcp/instructions.rs` — 변경 불필요
- `crates/secall-core/src/error.rs` — SecallError variant 추가 불필요 (anyhow::Result 사용)
- `crates/secall-core/src/store/` — DB/repo 레이어 변경 없음
- `crates/secall-core/src/search/` — 검색 레이어 변경 없음
- `obsidian-secall/` — Task 02 영역
