use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};

use super::instructions::build_instructions;
use super::tools::{GetParams, QueryType, RecallParams, StatusParams};
use crate::error::SecallError;
use crate::search::bm25::{SearchFilters, SearchResult};
use crate::search::hybrid::{parse_temporal_filter, SearchEngine};
use crate::store::db::Database;
use crate::store::SessionRepo;

fn to_mcp_error(e: SecallError) -> McpError {
    match &e {
        SecallError::SessionNotFound(_) | SecallError::TurnNotFound { .. } => {
            McpError::invalid_params(e.to_string(), None)
        }
        SecallError::DatabaseNotInitialized => McpError::internal_error(e.to_string(), None),
        SecallError::Parse { .. } | SecallError::UnsupportedFormat(_) => {
            McpError::invalid_params(e.to_string(), None)
        }
        _ => McpError::internal_error(e.to_string(), None),
    }
}

#[derive(Clone)]
pub struct SeCallMcpServer {
    tool_router: ToolRouter<Self>,
    db: Arc<Mutex<Database>>,
    search: Arc<SearchEngine>,
}

#[tool_router]
impl SeCallMcpServer {
    pub fn new(db: Arc<Mutex<Database>>, search: Arc<SearchEngine>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db,
            search,
        }
    }

    /// Search agent session history
    #[tool(
        description = "Search agent session history. Use keyword queries for exact terms, semantic queries for conceptual search, or temporal queries for time-based filtering."
    )]
    async fn recall(
        &self,
        Parameters(params): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = params.limit.unwrap_or(10).min(50);

        let mut base_filters = SearchFilters {
            project: params.project,
            agent: params.agent,
            since: None,
            until: None,
        };

        // Apply any temporal filters first
        for item in &params.queries {
            if let QueryType::Temporal = item.query_type {
                if let Some(tf) = parse_temporal_filter(&item.query) {
                    base_filters.since = tf.since;
                    base_filters.until = tf.until;
                }
            }
        }

        let mut all_results: Vec<SearchResult> = Vec::new();

        for item in &params.queries {
            match item.query_type {
                QueryType::Temporal => {}
                QueryType::Keyword => {
                    // BM25 search — sync, lock the DB
                    let results = {
                        let db = self.db.lock().map_err(|e| {
                            McpError::internal_error(format!("DB lock error: {e}"), None)
                        })?;
                        self.search
                            .search_bm25(&db, &item.query, &base_filters, limit)
                            .map_err(|e| {
                                McpError::internal_error(format!("BM25 error: {e}"), None)
                            })?
                    };
                    all_results.extend(results);
                }
                QueryType::Semantic => {
                    // Step 1: embed query (async, no DB lock held)
                    match self.search.embed_query(&item.query).await {
                        Ok(Some(embedding)) => {
                            // Step 2: lock DB and search vectors synchronously
                            let results = {
                                let db = self.db.lock().map_err(|e| {
                                    McpError::internal_error(format!("DB lock error: {e}"), None)
                                })?;
                                self.search
                                    .search_with_embedding(&db, &embedding, limit, &base_filters)
                                    .map_err(|e| McpError::internal_error(e.to_string(), None))?
                            };
                            all_results.extend(results);
                        }
                        Ok(None) => {
                            tracing::info!("vector search disabled (Ollama not available)");
                        }
                        Err(e) => {
                            return Err(McpError::internal_error(
                                format!("embedding failed: {e}"),
                                None,
                            ));
                        }
                    }
                }
            }
        }

        // Check if any keyword query was present
        let has_keyword = params
            .queries
            .iter()
            .any(|q| matches!(q.query_type, QueryType::Keyword));

        if !has_keyword && all_results.is_empty() {
            let json = serde_json::json!({ "results": [], "count": 0 });
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            )]));
        }

        // Deduplicate and sort
        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        all_results.retain(|r| seen.insert((r.session_id.clone(), r.turn_index)));
        all_results.truncate(limit);

        let count = all_results.len();
        let json = serde_json::json!({
            "results": all_results,
            "count": count
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }

    /// Get a specific session or turn by ID
    #[tool(
        description = "Retrieve a specific session or turn. Use session_id for full session metadata, session_id:N for a specific turn."
    )]
    fn get(&self, Parameters(params): Parameters<GetParams>) -> Result<CallToolResult, McpError> {
        let db = self
            .db
            .lock()
            .map_err(|e| McpError::internal_error(format!("DB lock error: {e}"), None))?;

        // Parse "session_id:turn_index" or just "session_id"
        let (session_id, turn_index) = if let Some(colon_pos) = params.id.rfind(':') {
            let sid = &params.id[..colon_pos];
            let tidx_str = &params.id[colon_pos + 1..];
            if let Ok(tidx) = tidx_str.parse::<u32>() {
                (sid.to_string(), Some(tidx))
            } else {
                (params.id.clone(), None)
            }
        } else {
            (params.id.clone(), None)
        };

        if let Some(turn_idx) = turn_index {
            match db.get_turn(&session_id, turn_idx) {
                Ok(turn) => {
                    let json_val = serde_json::json!({
                        "turn_index": turn.turn_index,
                        "role": turn.role,
                        "content": turn.content,
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&json_val).unwrap_or_default(),
                    )]))
                }
                Err(e) => Err(to_mcp_error(e)),
            }
        } else {
            match db.get_session_meta(&session_id) {
                Ok(meta) => {
                    let mut json_val = serde_json::to_value(&meta).unwrap_or_default();
                    if params.full.unwrap_or(false) {
                        if let Some(vault_path) = &meta.vault_path {
                            if let Ok(content) = std::fs::read_to_string(vault_path) {
                                json_val["content"] = serde_json::Value::String(content);
                            }
                        }
                    }
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&json_val).unwrap_or_default(),
                    )]))
                }
                Err(e) => Err(to_mcp_error(e)),
            }
        }
    }

    /// Show index health: session count, embedding status, recent ingests
    #[tool(description = "Show index health: session count, embedding status, recent ingests.")]
    fn status(&self, _params: Parameters<StatusParams>) -> String {
        let db = match self.db.lock() {
            Ok(d) => d,
            Err(e) => return format!("error: DB lock failed: {e}"),
        };
        match db.get_stats() {
            Ok(stats) => format!(
                "sessions: {}\nturns: {}\nvectors: {}\nrecent_ingests: {}",
                stats.session_count,
                stats.turn_count,
                stats.vector_count,
                stats.recent_ingests.len(),
            ),
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for SeCallMcpServer {
    fn get_info(&self) -> ServerInfo {
        let instructions = self
            .db
            .lock()
            .map(|db| build_instructions(&db))
            .unwrap_or_else(|_| "seCall — Agent Session Search Engine".to_string());

        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(instructions)
    }
}

pub async fn start_mcp_server(db: Database, search: SearchEngine) -> anyhow::Result<()> {
    let server = SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(search));
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let service = server.serve((stdin, stdout)).await?;
    service.waiting().await?;
    Ok(())
}

/// Start MCP server with HTTP/Streamable-HTTP transport (SSE-based).
pub async fn start_mcp_http_server(
    db: Database,
    search: SearchEngine,
    bind_addr: &str,
) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let db_arc = Arc::new(Mutex::new(db));
    let search_arc = Arc::new(search);

    let service: StreamableHttpService<SeCallMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || -> Result<SeCallMcpServer, std::io::Error> {
                Ok(SeCallMcpServer::new(db_arc.clone(), search_arc.clone()))
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );

    let router = axum::Router::new().nest_service("/mcp", service);
    let addr: std::net::SocketAddr = bind_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid bind address '{bind_addr}': {e}"))?;

    // Reject non-loopback addresses: no authentication is provided.
    if !addr.ip().is_loopback() {
        return Err(anyhow::anyhow!(
            "MCP HTTP server only allows loopback addresses (127.0.0.1 / ::1). \
             Got '{bind_addr}'. Binding to non-loopback interfaces would expose \
             an unauthenticated server to the network."
        ));
    }

    let tcp_listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(addr = %bind_addr, "MCP HTTP server listening");
    tracing::info!(endpoint = %format!("http://{bind_addr}/mcp"), "MCP endpoint");

    axum::serve(tcp_listener, router).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rmcp::handler::server::wrapper::Parameters;

    use super::super::tools::{QueryItem, QueryType, RecallParams, StatusParams};
    use super::SeCallMcpServer;
    use crate::search::bm25::Bm25Indexer;
    use crate::search::hybrid::SearchEngine;
    use crate::search::tokenizer::LinderaKoTokenizer;
    use crate::store::db::Database;

    fn make_server() -> SeCallMcpServer {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);
        SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(engine))
    }

    #[test]
    fn test_status_tool() {
        let server = make_server();
        let result = server.status(Parameters(StatusParams {}));
        assert!(
            result.contains("session") || result.contains("Session") || result.contains("error")
        );
    }

    #[tokio::test]
    async fn test_recall_empty_db() {
        let server = make_server();
        let params = RecallParams {
            queries: vec![QueryItem {
                query_type: QueryType::Keyword,
                query: "테스트 검색어".to_string(),
            }],
            project: None,
            agent: None,
            limit: Some(5),
        };
        let result = server.recall(Parameters(params)).await;
        assert!(result.is_ok());
    }
}
