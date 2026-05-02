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
use crate::search::bm25::{SearchFilters, SearchResult};
use crate::search::hybrid::{diversify_by_session, parse_temporal_filter, SearchEngine};
use crate::store::db::Database;
use crate::store::SessionRepo;

#[derive(Clone)]
pub struct SeCallMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    db: Arc<Mutex<Database>>,
    search: Arc<SearchEngine>,
    vault_path: PathBuf,
}

/// 공통 로직 메서드 — REST 핸들러와 MCP tool 모두에서 호출
impl SeCallMcpServer {
    pub fn new(db: Arc<Mutex<Database>>, search: Arc<SearchEngine>, vault_path: PathBuf) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db,
            search,
            vault_path,
        }
    }

    pub async fn do_recall(&self, params: RecallParams) -> anyhow::Result<serde_json::Value> {
        let limit = params.limit.unwrap_or(10).min(50);

        let mut base_filters = SearchFilters {
            project: params.project,
            agent: params.agent,
            since: None,
            until: None,
            exclude_session_types: vec!["automated".to_string()],
            ..Default::default()
        };

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
                    let results = {
                        let db = self
                            .db
                            .lock()
                            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
                        self.search
                            .search_bm25(&db, &item.query, &base_filters, limit)?
                    };
                    all_results.extend(results);
                }
                QueryType::Semantic => match self.search.embed_query(&item.query).await {
                    Ok(Some(embedding)) => {
                        let results = {
                            let db = self
                                .db
                                .lock()
                                .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
                            self.search.search_with_embedding(
                                &db,
                                &embedding,
                                limit,
                                &base_filters,
                            )?
                        };
                        all_results.extend(results);
                    }
                    Ok(None) => {
                        tracing::info!("vector search disabled (Ollama not available)");
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("embedding failed: {e}"));
                    }
                },
            }
        }

        let has_keyword = params
            .queries
            .iter()
            .any(|q| matches!(q.query_type, QueryType::Keyword));

        if !has_keyword && all_results.is_empty() {
            return Ok(serde_json::json!({ "results": [], "count": 0 }));
        }

        all_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut seen = std::collections::HashSet::new();
        all_results.retain(|r| seen.insert((r.session_id.clone(), r.turn_index)));

        let max_per = base_filters.max_per_session.unwrap_or(2);
        all_results = diversify_by_session(all_results, max_per);
        all_results.truncate(limit);

        let count = all_results.len();

        let related_sessions = {
            let db = self
                .db
                .lock()
                .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
            let seed_ids: Vec<&str> = all_results
                .iter()
                .map(|r| r.session_id.as_str())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            db.get_related_sessions(&seed_ids, 2, 5).unwrap_or_default()
        };

        Ok(serde_json::json!({
            "results": all_results,
            "count": count,
            "related_sessions": related_sessions,
        }))
    }

    pub fn do_get(&self, params: GetParams) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;

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
            let turn = db.get_turn(&session_id, turn_idx)?;
            Ok(serde_json::json!({
                "turn_index": turn.turn_index,
                "role": turn.role,
                "content": turn.content,
            }))
        } else {
            let meta = db.get_session_meta(&session_id)?;
            let mut json_val = serde_json::to_value(&meta).unwrap_or_default();
            // P32 Task 06 rework: 웹 UI/Obsidian가 SessionDetail에서 직접 사용하도록
            // tags/is_favorite/turn_count/start_time/id/summary를 추가. 기존 필드는 그대로 유지.
            if let Ok(item) = db.get_session_list_item(&session_id) {
                json_val["id"] = serde_json::Value::String(item.id);
                json_val["start_time"] = serde_json::Value::String(item.start_time);
                json_val["turn_count"] = serde_json::Value::Number(item.turn_count.into());
                json_val["is_favorite"] = serde_json::Value::Bool(item.is_favorite);
                json_val["tags"] = serde_json::to_value(&item.tags).unwrap_or_default();
                if let Some(s) = item.summary {
                    json_val["summary"] = serde_json::Value::String(s);
                }
                // P34 Task 00: notes 필드 보강
                json_val["notes"] = match item.notes {
                    Some(n) => serde_json::Value::String(n),
                    None => serde_json::Value::Null,
                };
            }
            // P34 Task 07: turn role 분포 + tool 사용 빈도 mini-chart용 통계.
            // 통계 조회 실패는 무시 (옵셔널 필드).
            if let Ok(stats) = db.get_session_stats(&session_id) {
                json_val["turn_role_counts"] = serde_json::json!({
                    "user": stats.user_turns,
                    "assistant": stats.assistant_turns,
                    "system": stats.system_turns,
                });
                json_val["tool_use_counts"] = serde_json::Value::Array(
                    stats
                        .tool_counts
                        .into_iter()
                        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
                        .collect(),
                );
            }
            if params.full.unwrap_or(false) {
                let content = if let Some(vault_path) = &meta.vault_path {
                    std::fs::read_to_string(vault_path).ok()
                } else {
                    None
                };
                // vault 파일이 없으면 DB turns를 합쳐 fallback content 생성
                let content = content.or_else(|| {
                    let mut stmt = db
                        .conn()
                        .prepare(
                            "SELECT role, content FROM turns WHERE session_id = ?1 ORDER BY turn_index",
                        )
                        .ok()?;
                    let rows: Vec<(String, String)> = stmt
                        .query_map(rusqlite::params![&session_id], |row| {
                            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                        })
                        .ok()?
                        .filter_map(|r| r.ok())
                        .collect();
                    if rows.is_empty() {
                        return None;
                    }
                    let mut buf = String::new();
                    for (role, text) in &rows {
                        buf.push_str(&format!("## {}\n\n{}\n\n", role, text));
                    }
                    Some(buf)
                });
                if let Some(c) = content {
                    json_val["content"] = serde_json::Value::String(c);
                }
            }
            Ok(json_val)
        }
    }

    pub fn do_status(&self) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
        let stats = db.get_stats()?;
        Ok(serde_json::json!({
            "sessions": stats.session_count,
            "turns": stats.turn_count,
            "vectors": stats.vector_count,
            "recent_ingests": stats.recent_ingests.len(),
        }))
    }

    /// 단일 위키 페이지 본문 반환 (`vault/wiki/projects/{safe_name}.md`).
    ///
    /// 파일이 없으면 `Err`을 반환하고 메시지에 `not found`를 포함시켜
    /// REST 핸들러가 404로 매핑할 수 있게 한다.
    pub fn do_wiki_get(&self, project: &str) -> anyhow::Result<serde_json::Value> {
        let safe = safe_project_name(project);
        if safe.is_empty() {
            return Err(anyhow::anyhow!(
                "wiki page not found for project: {project}"
            ));
        }

        let path = self
            .vault_path
            .join("wiki")
            .join("projects")
            .join(format!("{safe}.md"));

        if !path.exists() {
            return Err(anyhow::anyhow!(
                "wiki page not found for project: {project}"
            ));
        }

        let content = std::fs::read_to_string(&path)?;
        let updated = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

        Ok(serde_json::json!({
            "project": project,
            "path": path.to_string_lossy(),
            "content": content,
            "updated": updated,
        }))
    }

    pub fn do_wiki_search(&self, params: WikiSearchParams) -> anyhow::Result<serde_json::Value> {
        let wiki_dir = self.vault_path.join("wiki");
        let limit = params.limit.unwrap_or(5);
        let query_lower = params.query.to_lowercase();

        if !wiki_dir.exists() {
            return Ok(serde_json::json!({ "results": [], "count": 0 }));
        }

        let search_root = if let Some(ref cat) = params.category {
            match cat.as_str() {
                "projects" | "topics" | "decisions" => wiki_dir.join(cat),
                _ => {
                    return Err(anyhow::anyhow!(
                        "invalid category '{}': must be one of projects, topics, decisions",
                        cat
                    ));
                }
            }
        } else {
            wiki_dir.clone()
        };

        if !search_root.exists() {
            return Ok(serde_json::json!({ "results": [], "count": 0 }));
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
        Ok(serde_json::json!({ "results": results, "count": count }))
    }

    pub fn do_graph_query(&self, params: GraphQueryParams) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
        let depth = params.depth.unwrap_or(1).min(3);

        let neighbors = db.get_neighbors(&params.node_id)?;

        let filtered: Vec<_> = if let Some(ref rel) = params.relation {
            neighbors.into_iter().filter(|(_, r, _)| r == rel).collect()
        } else {
            neighbors
        };

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

        let results: Vec<serde_json::Value> = all_neighbors
            .iter()
            .map(|(id, rel, dir)| {
                let mut obj = serde_json::json!({
                    "node_id": id,
                    "relation": rel,
                    "direction": dir,
                });
                if let Ok(Some((node_type, label, _meta))) = db.get_node_metadata(id) {
                    obj["node_type"] = serde_json::Value::String(node_type);
                    obj["label"] = serde_json::Value::String(label);
                }
                obj
            })
            .collect();

        let count = results.len();
        Ok(serde_json::json!({
            "query_node": params.node_id,
            "depth": depth,
            "results": results,
            "count": count,
        }))
    }

    pub fn do_daily(&self, date: &str) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;

        let sessions = db.get_sessions_for_date(date)?;
        let total_sessions = sessions.len();

        // 자동화/노이즈 세션 필터링: 최소 2턴, automated 제외
        let meaningful: Vec<_> = sessions
            .iter()
            .filter(|(_, _, _, turns, _, stype)| *turns >= 2 && stype != "automated")
            .collect();

        // 노이즈 요약 필터링 (log.rs와 동일 기준)
        let noisy_prefixes = [
            "Analyze the following",
            "<environment_context>",
            "<local-command-caveat>",
        ];

        // 프로젝트별 그룹핑 + 노이즈 필터링 후 세션 ID 수집
        let mut by_project: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        let mut filtered_ids: Vec<String> = Vec::new();

        for (id, project, summary, turns, tools, _) in &meaningful {
            let summary_text = summary
                .as_deref()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(150)
                .collect::<String>();

            // 노이즈 요약 스킵
            if noisy_prefixes.iter().any(|p| summary_text.starts_with(p)) {
                continue;
            }

            filtered_ids.push(id.clone());
            let proj = project.as_deref().unwrap_or("(기타)").to_string();
            by_project.entry(proj).or_default().push(serde_json::json!({
                "session_id": id,
                "summary": summary_text,
                "turn_count": turns,
                "tools_used": tools.as_deref().unwrap_or("[]"),
            }));
        }

        // 토픽 조회 — 필터링 후 세션만 대상
        let topics = db.get_topics_for_sessions(&filtered_ids)?;
        let topic_labels: Vec<String> = topics
            .iter()
            .filter_map(|(_, t)| t.strip_prefix("topic:").map(|s| s.to_string()))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let filtered_sessions: usize = by_project.values().map(|v| v.len()).sum();

        Ok(serde_json::json!({
            "date": date,
            "total_sessions": total_sessions,
            "filtered_sessions": filtered_sessions,
            "topics": topic_labels,
            "projects": by_project,
        }))
    }

    // ─── REST listing / mutation (P32 Task 02) ──────────────────────────────

    pub fn do_list_sessions(
        &self,
        filter: crate::store::session_repo::SessionListFilter,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        let page = db.list_sessions_filtered(&filter)?;
        Ok(serde_json::to_value(page)?)
    }

    pub fn do_list_projects(&self) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        Ok(serde_json::json!({ "projects": db.list_projects()? }))
    }

    pub fn do_list_agents(&self) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        Ok(serde_json::json!({ "agents": db.list_agents()? }))
    }

    /// P35 Task 00: 전체 태그 목록 (빈도 포함/미포함).
    /// `with_counts=true` (기본): `{ "tags": [{ "name": "rust", "count": 12 }, ...] }`
    /// `with_counts=false`: `{ "tags": ["rust", "search", ...] }`
    pub fn do_list_tags(&self, with_counts: bool) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        let tags = db.list_all_tags()?;
        if with_counts {
            Ok(serde_json::json!({ "tags": tags }))
        } else {
            let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
            Ok(serde_json::json!({ "tags": names }))
        }
    }

    pub fn do_set_tags(
        &self,
        session_id: &str,
        tags: Vec<String>,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        let normalized = db.update_session_tags(session_id, &tags)?;
        Ok(serde_json::json!({ "session_id": session_id, "tags": normalized }))
    }

    pub fn do_set_favorite(
        &self,
        session_id: &str,
        favorite: bool,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        db.update_session_favorite(session_id, favorite)?;
        Ok(serde_json::json!({ "session_id": session_id, "favorite": favorite }))
    }

    /// P34 Task 00: 세션 노트 갱신. notes는 free-form markdown 문자열.
    /// `None` 또는 빈 문자열 모두 허용 (사용자 의도 보존).
    pub fn do_set_notes(
        &self,
        session_id: &str,
        notes: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        db.update_session_notes(session_id, notes)?;
        Ok(serde_json::json!({
            "session_id": session_id,
            "notes": notes,
        }))
    }

    // ─── Job 시스템 (P33 Task 03) ──────────────────────────────────────────

    /// 메모리 또는 DB에서 단일 job 상태 조회. 둘 다 없으면 Ok(None).
    ///
    /// 메모리 우선: 진행 중이거나 5분 보존 기간 내라면 `JobState`가 그대로 반환된다.
    /// 메모리에 없으면 DB의 `JobRow`를 `serde_json::Value`로 매핑해 반환한다 (kind/status는 문자열).
    pub async fn do_get_job(
        &self,
        executor: &crate::jobs::JobExecutor,
        id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if let Some(state) = executor.registry.get(id).await {
            return Ok(Some(serde_json::to_value(state)?));
        }
        let db = executor
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        match db.get_job(id)? {
            Some(row) => Ok(Some(serde_json::to_value(row)?)),
            None => Ok(None),
        }
    }

    /// 메모리에 있는 active jobs 목록 (started/running).
    pub async fn do_list_active_jobs(
        &self,
        executor: &crate::jobs::JobExecutor,
    ) -> anyhow::Result<serde_json::Value> {
        let states = executor.registry.list_active().await;
        Ok(serde_json::json!({ "jobs": states }))
    }

    /// DB에서 최근 job 기록 조회 (최대 50개).
    pub async fn do_list_recent_jobs(
        &self,
        executor: &crate::jobs::JobExecutor,
        limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let limit = limit.clamp(1, 200);
        let rows = {
            let db = executor
                .db
                .lock()
                .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
            db.list_recent_jobs(limit)?
        };
        Ok(serde_json::json!({ "jobs": rows }))
    }
}

/// MCP tool wrappers — 공통 do_*() 메서드를 CallToolResult로 래핑
#[tool_router]
impl SeCallMcpServer {
    #[tool(
        description = "Search agent session history. Use keyword queries for exact terms, semantic queries for conceptual search, or temporal queries for time-based filtering."
    )]
    async fn recall(
        &self,
        Parameters(params): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let json = self
            .do_recall(params)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Retrieve a specific session or turn. Use session_id for full session metadata, session_id:N for a specific turn."
    )]
    fn get(&self, Parameters(params): Parameters<GetParams>) -> Result<CallToolResult, McpError> {
        let json = self
            .do_get(params)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Show index health: session count, embedding status, recent ingests.")]
    fn status(&self, _params: Parameters<StatusParams>) -> String {
        match self.do_status() {
            Ok(json) => serde_json::to_string_pretty(&json).unwrap_or_default(),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(
        description = "Search wiki knowledge pages. Returns matching wiki articles from projects, topics, and decisions."
    )]
    fn wiki_search(
        &self,
        Parameters(params): Parameters<WikiSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let json = self
            .do_wiki_search(params)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json).unwrap_or_default(),
        )]))
    }

    #[tool(
        description = "Query the knowledge graph. Find neighbors and relationships of a node (session, project, agent, tool). Use depth to expand traversal. Returns connected nodes and edge types."
    )]
    fn graph_query(
        &self,
        Parameters(params): Parameters<GraphQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let json = self
            .do_graph_query(params)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
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

/// 프로젝트명을 wiki 파일명에 안전한 문자열로 정규화.
///
/// 위키 생성 측(`crates/secall/src/commands/wiki.rs::safe_project_name`)과 동일한 규칙:
/// 알파벳/숫자/`-`/`_`만 허용하고 그 외 문자는 `-`로 치환한 뒤 양 끝의 `-`를 제거한다.
fn safe_project_name(name: &str) -> String {
    name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "-")
        .trim_matches('-')
        .to_string()
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

    #[test]
    fn test_safe_project_name() {
        assert_eq!(super::safe_project_name("seCall"), "seCall");
        assert_eq!(super::safe_project_name("foo bar"), "foo-bar");
        assert_eq!(super::safe_project_name("a/b/c"), "a-b-c");
        assert_eq!(super::safe_project_name("--foo--"), "foo");
        assert_eq!(
            super::safe_project_name("한글_프로젝트-1"),
            "한글_프로젝트-1"
        );
    }

    #[test]
    fn test_do_wiki_get_returns_content() {
        use std::fs;

        let tmp = tempfile::tempdir().unwrap();
        let projects_dir = tmp.path().join("wiki").join("projects");
        fs::create_dir_all(&projects_dir).unwrap();
        let body = "# secall\n\n위키 본문 테스트.";
        fs::write(projects_dir.join("secall.md"), body).unwrap();

        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);
        let server = SeCallMcpServer::new(
            Arc::new(Mutex::new(db)),
            Arc::new(engine),
            tmp.path().to_path_buf(),
        );

        let v = server.do_wiki_get("secall").expect("ok");
        assert_eq!(v["project"], "secall");
        assert_eq!(v["content"].as_str().unwrap(), body);
        assert!(v["path"].as_str().unwrap().ends_with("secall.md"));
        assert!(v["updated"].as_str().is_some());
    }

    #[test]
    fn test_do_wiki_get_missing_returns_not_found_err() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);
        let server = SeCallMcpServer::new(
            Arc::new(Mutex::new(db)),
            Arc::new(engine),
            tmp.path().to_path_buf(),
        );

        let err = server.do_wiki_get("nope").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
