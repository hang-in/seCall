use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};

use super::instructions::build_instructions;
use super::tools::{
    GetParams, GraphQueryParams, QueryType, RecallParams, StatusParams, WikiSearchParams,
};
use crate::error::SecallError;
use crate::search::bm25::{SearchFilters, SearchResult};
use crate::search::hybrid::{diversify_by_session, parse_temporal_filter, SearchEngine};
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
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    db: Arc<Mutex<Database>>,
    search: Arc<SearchEngine>,
    vault_path: PathBuf,
}

#[tool_router]
impl SeCallMcpServer {
    pub fn new(db: Arc<Mutex<Database>>, search: Arc<SearchEngine>, vault_path: PathBuf) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db,
            search,
            vault_path,
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
            exclude_session_types: vec!["automated".to_string()],
            ..Default::default()
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

        // 세션 다양성 적용
        let max_per = base_filters.max_per_session.unwrap_or(2);
        all_results = diversify_by_session(all_results, max_per);

        all_results.truncate(limit);

        let count = all_results.len();

        // 관련 세션 그래프 탐색 (2홉, 최대 5개)
        let related_sessions = {
            let db = self
                .db
                .lock()
                .map_err(|e| McpError::internal_error(format!("DB lock error: {e}"), None))?;
            let seed_ids: Vec<&str> = all_results
                .iter()
                .map(|r| r.session_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            db.get_related_sessions(&seed_ids, 2, 5).unwrap_or_default()
        };

        let json = serde_json::json!({
            "results": all_results,
            "count": count,
            "related_sessions": related_sessions,
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

    /// Search wiki knowledge pages
    #[tool(
        description = "Search wiki knowledge pages. Returns matching wiki articles from projects, topics, and decisions."
    )]
    fn wiki_search(
        &self,
        Parameters(params): Parameters<WikiSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let wiki_dir = self.vault_path.join("wiki");
        let limit = params.limit.unwrap_or(5);
        let query_lower = params.query.to_lowercase();

        if !wiki_dir.exists() {
            let json = serde_json::json!({ "results": [], "count": 0 });
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            )]));
        }

        // wiki/ 하위 MD 파일 수집 (category 필터 적용)
        // category는 허용 목록으로 검증 — 임의 경로 탐색 방지
        let search_root = if let Some(ref cat) = params.category {
            match cat.as_str() {
                "projects" | "topics" | "decisions" => wiki_dir.join(cat),
                _ => {
                    return Err(McpError::invalid_params(
                        format!(
                            "invalid category '{}': must be one of projects, topics, decisions",
                            cat
                        ),
                        None,
                    ));
                }
            }
        } else {
            wiki_dir.clone()
        };

        if !search_root.exists() {
            let json = serde_json::json!({ "results": [], "count": 0 });
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            )]));
        }

        struct Match {
            path: String,
            title: String,
            preview: String,
            name_match: bool,
            created: Option<String>,
            updated: Option<String>,
        }

        let mut matches: Vec<Match> = walkdir::WalkDir::new(&search_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
            .filter_map(|entry| {
                let path = entry.path();
                let filename = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                let content = std::fs::read_to_string(path).ok()?;
                let content_lower = content.to_lowercase();

                let name_match = filename.contains(&query_lower);
                let body_match = content_lower.contains(&query_lower);

                if !name_match && !body_match {
                    return None;
                }

                let rel = path
                    .strip_prefix(&self.vault_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                // title: 첫 번째 # 헤더 or 파일명
                let title = content
                    .lines()
                    .find(|l| l.starts_with("# "))
                    .map(|l| l.trim_start_matches("# ").to_string())
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    });

                let preview: String = content.chars().take(500).collect();

                // frontmatter에서 created/updated 추출
                let (created, updated) = extract_wiki_dates(&content);

                Some(Match {
                    path: rel,
                    title,
                    preview,
                    name_match,
                    created,
                    updated,
                })
            })
            .collect();

        // 파일명 매칭을 우선 정렬
        matches.sort_by_key(|m| !m.name_match);
        matches.truncate(limit);

        let results: Vec<serde_json::Value> = matches
            .into_iter()
            .map(|m| {
                let mut obj = serde_json::json!({
                    "path": m.path,
                    "title": m.title,
                    "preview": m.preview,
                });
                if let Some(created) = m.created {
                    obj["created"] = serde_json::Value::String(created);
                }
                if let Some(updated) = m.updated {
                    obj["updated"] = serde_json::Value::String(updated);
                }
                obj
            })
            .collect();

        let count = results.len();
        let json = serde_json::json!({ "results": results, "count": count });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }

    /// Query knowledge graph: find neighbors and relationships of a node
    #[tool(
        description = "Query the knowledge graph. Find neighbors and relationships of a node (session, project, agent, tool). Use depth to expand traversal. Returns connected nodes and edge types."
    )]
    fn graph_query(
        &self,
        Parameters(params): Parameters<GraphQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self
            .db
            .lock()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let depth = params.depth.unwrap_or(1).min(3); // 최대 3홉

        // 1홉 이웃 조회
        let neighbors = db.get_neighbors(&params.node_id).map_err(to_mcp_error)?;

        // relation 필터 적용
        let filtered: Vec<_> = if let Some(ref rel) = params.relation {
            neighbors.into_iter().filter(|(_, r, _)| r == rel).collect()
        } else {
            neighbors
        };

        // depth > 1이면 BFS로 확장 (2홉, 3홉)
        let mut all_neighbors = filtered.clone();
        if depth > 1 {
            let mut visited = std::collections::HashSet::new();
            visited.insert(params.node_id.clone());
            let mut frontier: Vec<String> = filtered.iter().map(|(id, _, _)| id.clone()).collect();

            for _ in 1..depth {
                let mut next_frontier = Vec::new();
                for node in &frontier {
                    if visited.contains(node) {
                        continue;
                    }
                    visited.insert(node.clone());
                    if let Ok(nb) = db.get_neighbors(node) {
                        let nb_filtered: Vec<_> = if let Some(ref rel) = params.relation {
                            nb.into_iter().filter(|(_, r, _)| r == rel).collect()
                        } else {
                            nb
                        };
                        for n in &nb_filtered {
                            next_frontier.push(n.0.clone());
                        }
                        all_neighbors.extend(nb_filtered);
                    }
                }
                frontier = next_frontier;
            }
        }

        // 결과 JSON
        let results: Vec<serde_json::Value> = all_neighbors
            .iter()
            .map(|(id, rel, dir)| {
                serde_json::json!({
                    "node_id": id,
                    "relation": rel,
                    "direction": dir,
                })
            })
            .collect();

        let count = results.len();
        let json = serde_json::json!({
            "query_node": params.node_id,
            "depth": depth,
            "results": results,
            "count": count,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
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

pub async fn start_mcp_server(
    db: Database,
    search: SearchEngine,
    vault_path: PathBuf,
) -> anyhow::Result<()> {
    let server = SeCallMcpServer::new(Arc::new(Mutex::new(db)), Arc::new(search), vault_path);
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let service = server.serve((stdin, stdout)).await?;
    service.waiting().await?;
    Ok(())
}

/// Start MCP server with HTTP/Streamable-HTTP transport (SSE-based).
pub async fn start_mcp_http_server(
    db: Database,
    search: SearchEngine,
    vault_path: PathBuf,
    bind_addr: &str,
) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };

    let db_arc = Arc::new(Mutex::new(db));
    let search_arc = Arc::new(search);
    let vault_path_arc = Arc::new(vault_path);

    let service: StreamableHttpService<SeCallMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            move || -> Result<SeCallMcpServer, std::io::Error> {
                Ok(SeCallMcpServer::new(
                    db_arc.clone(),
                    search_arc.clone(),
                    (*vault_path_arc).clone(),
                ))
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

/// wiki md frontmatter에서 created/updated 값을 추출.
fn extract_wiki_dates(content: &str) -> (Option<String>, Option<String>) {
    let fm = match content.strip_prefix("---\n") {
        Some(rest) => match rest.split_once("\n---") {
            Some((fm, _)) => fm,
            None => return (None, None),
        },
        None => return (None, None),
    };

    let mut created = None;
    let mut updated = None;
    for line in fm.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("created:") {
            created = Some(val.trim().trim_matches('"').to_string());
        } else if let Some(val) = trimmed.strip_prefix("updated:") {
            updated = Some(val.trim().trim_matches('"').to_string());
        }
    }
    (created, updated)
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
        SeCallMcpServer::new(
            Arc::new(Mutex::new(db)),
            Arc::new(engine),
            std::path::PathBuf::from("/tmp/secall-test-vault"),
        )
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

    #[test]
    fn test_extract_wiki_dates_both() {
        let content = "---\ntitle: Test\ncreated: 2026-04-10\nupdated: 2026-04-12\n---\n# Test";
        let (created, updated) = super::extract_wiki_dates(content);
        assert_eq!(created.as_deref(), Some("2026-04-10"));
        assert_eq!(updated.as_deref(), Some("2026-04-12"));
    }

    #[test]
    fn test_extract_wiki_dates_none() {
        let content = "---\ntitle: Test\n---\n# Test";
        let (created, updated) = super::extract_wiki_dates(content);
        assert!(created.is_none());
        assert!(updated.is_none());
    }

    #[test]
    fn test_extract_wiki_dates_no_frontmatter() {
        let content = "# Just a heading\nSome text";
        let (created, updated) = super::extract_wiki_dates(content);
        assert!(created.is_none());
        assert!(updated.is_none());
    }

    #[test]
    fn test_extract_wiki_dates_quoted() {
        let content = "---\ncreated: \"2026-04-10\"\n---\n";
        let (created, _) = super::extract_wiki_dates(content);
        assert_eq!(created.as_deref(), Some("2026-04-10"));
    }
}
