---
type: task
status: ready
plan: p25-obsidian-plugin
task_number: 1
title: REST API 서버 (secall serve)
updated_at: 2026-04-14
---

# Task 01 — REST API 서버 (`secall serve`)

## 목표

MCP tool 로직을 공통 레이어로 추출하고, REST 엔드포인트를 추가하여
`secall serve --port 8080`으로 REST + MCP를 동시에 서빙한다.

## 아키텍처

```
SeCallMcpServer
├── do_recall()  ──┐
├── do_get()     ──┤── 공통 로직 (Result<Value>)
├── do_status()  ──┤
├── do_wiki()    ──┤
├── do_graph()   ──┘
│
├── #[tool] recall()      → CallToolResult  (MCP 진입점)
├── #[tool] get()         → CallToolResult
│   ...
│
├── REST handlers (axum)
│   ├── POST /api/recall  → Json<Value>
│   ├── POST /api/get     → Json<Value>
│   ├── GET  /api/status  → Json<Value>
│   ├── POST /api/wiki    → Json<Value>
│   └── POST /api/graph   → Json<Value>
│
└── /mcp                  → 기존 MCP (StreamableHttpService)
```

## Changed files

### 1. `crates/secall-core/src/mcp/server.rs` (수정)

**핵심 변경**: 5개 MCP tool에서 핵심 로직을 `pub` 메서드로 추출.

현재 각 tool 메서드는:
1. params에서 값 추출
2. db/search 호출 → serde_json::Value 생성
3. CallToolResult::success(Content::text(...)) 로 래핑

변경 후:
```rust
impl SeCallMcpServer {
    // --- 공통 로직 (pub) ---

    pub async fn do_recall(&self, params: RecallParams) -> Result<serde_json::Value, SecallError> {
        // 기존 recall() 내부 로직 그대로 이동
        // McpError 대신 SecallError 반환
        // 마지막에 json!({...}) 반환 (CallToolResult 래핑 안 함)
    }

    pub fn do_get(&self, params: GetParams) -> Result<serde_json::Value, SecallError> { ... }

    pub fn do_status(&self) -> Result<serde_json::Value, SecallError> {
        // 기존 status()는 String 반환 → JSON으로 변경
        let stats = db.get_stats()?;
        Ok(json!({
            "sessions": stats.session_count,
            "turns": stats.turn_count,
            "vectors": stats.vector_count,
            "recent_ingests": stats.recent_ingests,
        }))
    }

    pub fn do_wiki_search(&self, params: WikiSearchParams) -> Result<serde_json::Value, SecallError> { ... }
    pub fn do_graph_query(&self, params: GraphQueryParams) -> Result<serde_json::Value, SecallError> { ... }

    // --- MCP tool wrappers (기존 #[tool] 메서드) ---

    #[tool(...)]
    async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> Result<CallToolResult, McpError> {
        let json = self.do_recall(params).await.map_err(to_mcp_error)?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }
    // ... get, status, wiki_search, graph_query 동일 패턴
}
```

**에러 변환 주의**:
- 기존 MCP tools에서 `self.db.lock().map_err(|e| McpError::...)` 사용 중
- `do_*` 메서드에서는 `SecallError`로 통일: DB lock 실패 → `SecallError::Database(...)`
- `SecallError`에 lock 실패 variant가 없으면 `anyhow::Error`로 변환하거나 `SecallError::Other` 사용

**현재 SecallError variants 확인 필요**: `crates/secall-core/src/error.rs`에서 적절한 variant 확인.
DB lock 실패를 감싸 반환할 variant가 없으면 추가하지 말고, `anyhow::Result<serde_json::Value>`로 반환 타입 사용.

### 2. `crates/secall-core/src/mcp/rest.rs` (신규)

REST 핸들러 + axum Router 정의.

```rust
use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use super::server::SeCallMcpServer;
use super::tools::{RecallParams, GetParams, WikiSearchParams, GraphQueryParams};

type AppState = Arc<SeCallMcpServer>;

pub fn rest_router(server: SeCallMcpServer) -> Router {
    let state: AppState = Arc::new(server);
    Router::new()
        .route("/api/recall", post(api_recall))
        .route("/api/get", post(api_get))
        .route("/api/status", get(api_status))
        .route("/api/wiki", post(api_wiki))
        .route("/api/graph", post(api_graph))
        .with_state(state)
}

async fn api_recall(
    State(server): State<AppState>,
    Json(params): Json<RecallParams>,
) -> impl IntoResponse {
    match server.do_recall(params).await {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

// api_get, api_status, api_wiki, api_graph 동일 패턴

fn error_response(e: impl std::fmt::Display) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": e.to_string()})),
    ).into_response()
}
```

**CORS**: Obsidian `app://obsidian.md` origin 허용 필요.
tower-http의 CorsLayer 사용:
```rust
use tower_http::cors::{CorsLayer, Any};
let cors = CorsLayer::new()
    .allow_origin(Any)  // Phase 1에서는 Any, 추후 제한 가능
    .allow_methods(Any)
    .allow_headers(Any);
```

**tower-http 의존성 추가 필요** → Cargo.toml 수정 (아래 참조).

### 3. `crates/secall-core/src/mcp/mod.rs` (수정)

```rust
pub mod instructions;
pub mod rest;       // ← 추가
pub mod server;
pub mod tools;

pub use server::{start_mcp_http_server, start_mcp_server, SeCallMcpServer};
pub use rest::start_rest_server;  // ← 추가
```

### 4. `crates/secall-core/Cargo.toml` (수정)

tower-http 의존성 추가:
```toml
[dependencies]
tower-http = { version = "0.6", features = ["cors"] }
```

워크스페이스 Cargo.toml에도 추가:
```toml
tower-http = { version = "0.6", features = ["cors"] }
```

### 5. `crates/secall/src/commands/serve.rs` (신규)

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

### 6. `crates/secall/src/commands/mod.rs` (수정)

```rust
pub mod serve;  // ← 추가
```

### 7. `crates/secall/src/main.rs` (수정)

Commands enum에 Serve variant 추가:
```rust
/// Start REST + MCP API server
Serve {
    /// Port number (default: 8080)
    #[arg(long, short, default_value = "8080")]
    port: u16,
},
```

match arm 추가:
```rust
Commands::Serve { port } => {
    commands::serve::run(port).await?;
}
```

## Dependencies

없음 (첫 번째 Task)

## Verification

```bash
# 1. 타입 체크
cargo check 2>&1 | tail -5

# 2. 기존 테스트 통과 (MCP 리팩터링이 기존 동작을 깨지 않는지 확인)
cargo test 2>&1 | tail -10

# 3. serve 서버 기동 + REST API 호출
# 터미널 1:
# secall serve --port 8080
# 터미널 2:
# curl -s http://127.0.0.1:8080/api/status | head -5
# curl -s -X POST http://127.0.0.1:8080/api/recall -H 'Content-Type: application/json' -d '{"queries":[{"type":"keyword","query":"rust"}]}' | head -20
# Manual: secall serve --port 8080을 실행하고, 위 curl 명령으로 JSON 응답 확인
```

## Risks

- **MCP tool 리팩터링**: `#[tool_router]` / `#[tool]` 매크로가 메서드 시그니처에 민감할 수 있음. `do_*` 메서드 추가 시 매크로가 간섭하지 않는지 확인 필요.
- **DB lock 에러 타입 변환**: `Mutex::lock()` 실패 → SecallError 변환이 자연스럽지 않을 수 있음. `anyhow::Error`로 우회 가능.
- **tower-http 버전 호환**: axum 0.8과 호환되는 tower-http 버전 확인 필요 (0.6.x 예상).
