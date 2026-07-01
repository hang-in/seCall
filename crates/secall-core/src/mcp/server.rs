use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};

use super::instructions::build_instructions;
use super::tools::{
    GetParams, GraphQueryParams, QueryType, RecallParams, StatusParams, WikiSearchMode,
    WikiSearchParams,
};
use crate::search::bm25::{SearchFilters, SearchResult};
use crate::search::hybrid::{diversify_by_session, parse_temporal_filter, SearchEngine};
use crate::search::{Embedder, OllamaEmbedder};
use crate::store::db::Database;
use crate::store::{SessionRepo, WikiVectorRepo};
use crate::vault::Config;

#[derive(Clone)]
pub struct SeCallMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    db: Arc<Mutex<Database>>,
    search: Arc<SearchEngine>,
    vault_path: PathBuf,
    allow_config_edit: bool,
}

#[derive(Clone)]
struct WikiMatch {
    path: String,
    title: String,
    preview: String,
    name_match: bool,
    created: Option<String>,
    updated: Option<String>,
    score: f32,
}

fn run_future_blocking<T, F>(future: F) -> anyhow::Result<T>
where
    T: Send,
    F: std::future::Future<Output = anyhow::Result<T>> + Send,
{
    std::thread::scope(|scope| {
        let task = scope.spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(future)
        });
        task.join()
            .map_err(|_| anyhow::anyhow!("wiki search worker thread panicked"))?
    })
}

/// 공통 로직 메서드 — REST 핸들러와 MCP tool 모두에서 호출
impl SeCallMcpServer {
    pub fn new(db: Arc<Mutex<Database>>, search: Arc<SearchEngine>, vault_path: PathBuf) -> Self {
        Self::new_with_options(db, search, vault_path, false)
    }

    pub fn new_with_options(
        db: Arc<Mutex<Database>>,
        search: Arc<SearchEngine>,
        vault_path: PathBuf,
        allow_config_edit: bool,
    ) -> Self {
        Self {
            tool_router: Self::tool_router(),
            db,
            search,
            vault_path,
            allow_config_edit,
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
            // P62: web TopNav 의 version 표시를 server 의 빌드 시점 버전으로
            // 통합 (이전엔 web 측 hardcode 가 v0.4.2 로 고정돼 있었음).
            // env!() 는 secall-core 의 CARGO_PKG_VERSION — workspace.package
            // 버전과 동일하게 따라간다.
            "version": env!("CARGO_PKG_VERSION"),
            "sessions": stats.session_count,
            "turns": stats.turn_count,
            "vectors": stats.vector_count,
            "recent_ingests": stats.recent_ingests.len(),
        }))
    }

    pub fn do_config_get(&self) -> anyhow::Result<serde_json::Value> {
        let config = Config::load_or_default();
        let mut json = serde_json::to_value(config)?;
        for section_key in ["graph", "log", "embedding"] {
            if let Some(section) = json.get_mut(section_key).and_then(|v| v.as_object_mut()) {
                if section.get("cloud_api_key").is_some() {
                    section.insert(
                        "cloud_api_key".to_string(),
                        serde_json::Value::String("<masked>".to_string()),
                    );
                }
            }
        }
        if let Some(root) = json.as_object_mut() {
            root.insert(
                "env_indicators".to_string(),
                serde_json::json!({
                    "ANTHROPIC_API_KEY": std::env::var("ANTHROPIC_API_KEY").is_ok(),
                    "OLLAMA_CLOUD_API_KEY": std::env::var("OLLAMA_CLOUD_API_KEY").is_ok(),
                    "OPENAI_API_KEY": std::env::var("OPENAI_API_KEY").is_ok(),
                }),
            );
        }
        Ok(json)
    }

    pub fn do_config_patch(
        &self,
        section: &str,
        body: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        if !self.allow_config_edit {
            anyhow::bail!("config edit disabled");
        }

        let patch = body
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("config patch body must be a JSON object"))?;
        let mut config = Config::load_or_default();

        match section {
            "wiki" => {
                let mut current = serde_json::to_value(&config.wiki)?;
                merge_json_object(&mut current, &serde_json::Value::Object(patch.clone()));
                config.wiki = serde_json::from_value(current)?;
            }
            "graph" => {
                let mut sanitized_patch = serde_json::Map::new();
                for (key, value) in patch {
                    if key != "cloud_api_key" {
                        sanitized_patch.insert(key.clone(), value.clone());
                    }
                }
                let mut current = serde_json::to_value(&config.graph)?;
                merge_json_object(&mut current, &serde_json::Value::Object(sanitized_patch));
                config.graph = serde_json::from_value(current)?;
            }
            "log" => {
                let mut sanitized_patch = serde_json::Map::new();
                for (key, value) in patch {
                    if key != "cloud_api_key" {
                        sanitized_patch.insert(key.clone(), value.clone());
                    }
                }
                let mut current = serde_json::to_value(&config.log)?;
                merge_json_object(&mut current, &serde_json::Value::Object(sanitized_patch));
                config.log = serde_json::from_value(current)?;
            }
            "embedding" => {
                let mut sanitized_patch = serde_json::Map::new();
                for (key, value) in patch {
                    if key != "cloud_api_key" {
                        sanitized_patch.insert(key.clone(), value.clone());
                    }
                }
                let mut current = serde_json::to_value(&config.embedding)?;
                merge_json_object(&mut current, &serde_json::Value::Object(sanitized_patch));
                config.embedding = serde_json::from_value(current)?;
            }
            _ => anyhow::bail!("unknown config section: {section}"),
        }

        config.save()?;
        self.do_config_get()
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

    /// `vault/wiki/projects/*.md` 디렉토리를 스캔해 실제 존재하는 wiki 페이지 목록 반환.
    /// 응답 형태: `{ "projects": [{"project": "<safe_name>", "updated": "<rfc3339>"}, ...], "count": N }`
    /// — secall-web 의 좌측 wiki 리스트가 sessions DB 의 distinct project 가 아닌
    ///   실제 wiki 페이지 기준으로 표시되도록 분리된 endpoint.
    /// 의미 있는 그래프 subset 한 번에 반환 (Stage 9).
    ///
    /// - project / topic / agent / tool 노드는 전부
    /// - session 노드는 degree 상위 `session_limit` 개만 (default 80)
    /// - 위 노드 ID 집합 안의 엣지를 우선순위별 sort 후 `edge_limit` 까지
    ///
    /// P64: `edge_limit` 추가. 이전엔 모든 in-set edges 가 응답됨 → 사용자 환경
    /// 에서 1246 edges / ~600KB 응답으로 web force-simulation 이 멈춤 화면.
    ///
    /// 우선순위 (priority 0 = 최상위):
    ///
    /// - 0: session ↔ session 관계 (same_project / same_day / cluster_*) —
    ///   그래프 구조 파악에 가장 valuable
    /// - 1: session → topic / tool (rich label)
    /// - 2: session → agent / project (cardinality 높아 중복적)
    /// - 3: 그 외
    ///
    /// 같은 우선순위 내에선 입력 순서 유지 (DB rowid 기준 stable).
    ///
    /// 응답: `{"nodes": [...], "edges": [...], "node_count", "edge_count",
    /// "session_limit", "edge_limit", "edges_truncated": bool}`
    pub fn do_graph_snapshot(
        &self,
        session_limit: usize,
        edge_limit: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;

        let mut all_nodes: Vec<(String, String, String)> = Vec::new();
        for t in &["project", "topic", "agent", "tool"] {
            all_nodes.extend(db.list_graph_nodes(Some(t))?);
        }

        // session 노드: degree (in + out) 상위 N
        let mut stmt = db.conn().prepare(
            "SELECT n.id, n.type, n.label,
                    (SELECT COUNT(*) FROM graph_edges WHERE source = n.id OR target = n.id) AS deg
             FROM graph_nodes n WHERE n.type = 'session'
             ORDER BY deg DESC LIMIT ?1",
        )?;
        let session_rows: Vec<(String, String, String)> = stmt
            .query_map([session_limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        all_nodes.extend(session_rows);

        // node id → type 매핑: 우선순위 결정 + in-set 존재 여부 동시 처리 (Gemini PR #76 리뷰 반영).
        // `HashMap<&str, &str>` 는 stable Rust 에서 `get(&str)` 의 trait resolution
        // (issue #130366) 으로 어색하므로 K=String 유지, 별도 id_set 만 제거.
        let type_of: std::collections::HashMap<String, String> = all_nodes
            .iter()
            .map(|(id, ty, _)| (id.clone(), ty.clone()))
            .collect();

        let mut stmt = db
            .conn()
            .prepare("SELECT source, target, relation FROM graph_edges")?;
        let mut edges: Vec<(String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter(|(s, t, _)| type_of.contains_key(s) && type_of.contains_key(t))
            .collect();

        let total_in_set = edges.len();

        // 우선순위: session-session > session-(topic/tool) > session-(agent/project) > 그 외
        let priority = |s: &str, t: &str| -> u8 {
            let ts = type_of.get(s).map(String::as_str).unwrap_or("");
            let tt = type_of.get(t).map(String::as_str).unwrap_or("");
            let ss = ts == "session" && tt == "session";
            if ss {
                return 0;
            }
            let touches_session = ts == "session" || tt == "session";
            if touches_session {
                let other = if ts == "session" { tt } else { ts };
                return match other {
                    "topic" | "tool" => 1,
                    "agent" | "project" => 2,
                    _ => 3,
                };
            }
            3
        };

        // stable sort_by_key: 같은 우선순위는 DB 원래 순서 유지
        edges.sort_by_key(|(s, t, _)| priority(s, t));
        let edges_truncated = edges.len() > edge_limit;
        edges.truncate(edge_limit);

        Ok(serde_json::json!({
            "nodes": all_nodes
                .iter()
                .map(|(id, ty, lbl)| serde_json::json!({"id": id, "type": ty, "label": lbl}))
                .collect::<Vec<_>>(),
            "edges": edges
                .iter()
                .map(|(s, t, r)| serde_json::json!({"source": s, "target": t, "relation": r}))
                .collect::<Vec<_>>(),
            "node_count": all_nodes.len(),
            "edge_count": edges.len(),
            "total_edges_in_set": total_in_set,
            "session_limit": session_limit,
            "edge_limit": edge_limit,
            "edges_truncated": edges_truncated,
        }))
    }

    pub fn do_wiki_list(&self) -> anyhow::Result<serde_json::Value> {
        let projects_dir = self.vault_path.join("wiki").join("projects");
        if !projects_dir.exists() {
            return Ok(serde_json::json!({"projects": [], "count": 0}));
        }

        let mut projects: Vec<serde_json::Value> = Vec::new();
        for entry in std::fs::read_dir(&projects_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            // `.md` 확장자를 가진 디렉토리가 들어올 가능성 차단 (실제 파일만 통과).
            if !path.is_file() || !path.extension().map(|e| e == "md").unwrap_or(false) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let updated = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());
            projects.push(serde_json::json!({
                "project": stem,
                "updated": updated,
            }));
        }

        projects.sort_by(|a, b| {
            a["project"]
                .as_str()
                .unwrap_or("")
                .cmp(b["project"].as_str().unwrap_or(""))
        });

        let count = projects.len();
        Ok(serde_json::json!({
            "projects": projects,
            "count": count,
        }))
    }

    pub fn do_wiki_search(&self, params: WikiSearchParams) -> anyhow::Result<serde_json::Value> {
        let mode = params.mode.unwrap_or_default();
        let matches = match mode {
            WikiSearchMode::Keyword => self.do_wiki_search_keyword(&params),
            WikiSearchMode::Semantic => self.do_wiki_search_semantic(&params).or_else(|err| {
                tracing::warn!(error = %err, "semantic wiki search failed, falling back to keyword");
                self.do_wiki_search_keyword(&params)
            }),
            WikiSearchMode::Hybrid => self.do_wiki_search_hybrid(&params).or_else(|err| {
                tracing::warn!(error = %err, "hybrid wiki search failed, falling back to keyword");
                self.do_wiki_search_keyword(&params)
            }),
        }?;

        Ok(wiki_matches_to_json(matches))
    }

    fn do_wiki_search_keyword(&self, params: &WikiSearchParams) -> anyhow::Result<Vec<WikiMatch>> {
        let limit = params.limit.unwrap_or(5);
        let mut matches = self.collect_keyword_matches(params)?;
        matches.sort_by(|a, b| {
            b.name_match
                .cmp(&a.name_match)
                .then_with(|| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.path.cmp(&b.path))
        });
        matches.truncate(limit);
        Ok(matches)
    }

    fn do_wiki_search_semantic(&self, params: &WikiSearchParams) -> anyhow::Result<Vec<WikiMatch>> {
        let limit = params.limit.unwrap_or(5);
        let mut matches = self.collect_semantic_matches(params)?;
        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.name_match.cmp(&a.name_match))
                .then_with(|| a.path.cmp(&b.path))
        });
        matches.truncate(limit);
        Ok(matches)
    }

    fn do_wiki_search_hybrid(&self, params: &WikiSearchParams) -> anyhow::Result<Vec<WikiMatch>> {
        let limit = params.limit.unwrap_or(5);
        let keyword = self.collect_keyword_matches(params)?;
        let semantic = self.collect_semantic_matches(params)?;
        let mut merged = std::collections::HashMap::<String, WikiMatch>::new();
        let mut rrf_scores = std::collections::HashMap::<String, f32>::new();

        for (rank, item) in keyword.iter().enumerate() {
            *rrf_scores.entry(item.path.clone()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
            merged
                .entry(item.path.clone())
                .or_insert_with(|| item.clone());
        }
        for (rank, item) in semantic.iter().enumerate() {
            *rrf_scores.entry(item.path.clone()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
            merged
                .entry(item.path.clone())
                .and_modify(|existing| {
                    existing.name_match = existing.name_match || item.name_match;
                    if existing.created.is_none() {
                        existing.created = item.created.clone();
                    }
                    if existing.updated.is_none() {
                        existing.updated = item.updated.clone();
                    }
                })
                .or_insert_with(|| item.clone());
        }

        let mut matches: Vec<WikiMatch> = merged
            .into_iter()
            .filter_map(|(path, mut item)| {
                item.score = *rrf_scores.get(&path)?;
                Some(item)
            })
            .collect();

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.name_match.cmp(&a.name_match))
                .then_with(|| a.path.cmp(&b.path))
        });
        matches.truncate(limit);
        Ok(matches)
    }

    fn collect_keyword_matches(&self, params: &WikiSearchParams) -> anyhow::Result<Vec<WikiMatch>> {
        let wiki_dir = self.vault_path.join("wiki");
        if !wiki_dir.exists() {
            return Ok(Vec::new());
        }

        let search_root = wiki_search_root(&self.vault_path, params.category.as_deref())?;
        if !search_root.exists() {
            return Ok(Vec::new());
        }

        let query_lower = params.query.to_lowercase();

        let matches = walkdir::WalkDir::new(&search_root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "md")
                    .unwrap_or(false)
            })
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

                Some(build_wiki_match(
                    &self.vault_path,
                    path,
                    &content,
                    name_match,
                    if name_match { 2.0 } else { 1.0 },
                ))
            })
            .collect();

        Ok(matches)
    }

    fn collect_semantic_matches(
        &self,
        params: &WikiSearchParams,
    ) -> anyhow::Result<Vec<WikiMatch>> {
        let wiki_dir = self.vault_path.join("wiki");
        if !wiki_dir.exists() {
            return Ok(Vec::new());
        }

        let query_embedding = {
            let base_url = std::env::var("OLLAMA_BASE_URL").ok();
            let model = std::env::var("OLLAMA_EMBED_MODEL").ok();
            let embedder = OllamaEmbedder::new(base_url.as_deref(), model.as_deref());
            run_future_blocking(embedder.embed(&params.query))?
        };

        let category_prefix = params
            .category
            .as_deref()
            .map(validated_wiki_category)
            .transpose()?
            .map(|category| format!("wiki/{category}/"));

        let rows = {
            let db = self
                .db
                .lock()
                .map_err(|e| anyhow::anyhow!("DB lock: {e}"))?;
            db.list_wiki_vectors()?
        };

        let query_lower = params.query.to_lowercase();
        let mut matches = Vec::new();

        for row in rows {
            if let Some(prefix) = category_prefix.as_deref() {
                if !row.wiki_path.starts_with(prefix) {
                    continue;
                }
            }
            if row.embedding.is_empty() {
                continue;
            }

            let full_path = self.vault_path.join(&row.wiki_path);
            let content = match std::fs::read_to_string(&full_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let score =
                crate::store::wiki_vector_repo::cosine_similarity(&query_embedding, &row.embedding);
            let name_match = full_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query_lower);

            matches.push(build_wiki_match(
                &self.vault_path,
                &full_path,
                &content,
                name_match,
                score,
            ));
        }

        Ok(matches)
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

    /// 세션 완전 삭제 — sessions / turns / turns_fts / turn_vectors 전부 제거.
    /// Web UI 세션 삭제 버튼용. 되돌릴 수 없음.
    pub fn do_delete_session(&self, session_id: &str) -> anyhow::Result<serde_json::Value> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        db.delete_session_full(session_id)?;
        Ok(serde_json::json!({ "session_id": session_id, "deleted": true }))
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

fn merge_json_object(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let Some(target_obj) = target.as_object_mut() else {
        *target = patch.clone();
        return;
    };
    let Some(patch_obj) = patch.as_object() else {
        *target = patch.clone();
        return;
    };

    for (key, value) in patch_obj {
        match (target_obj.get_mut(key), value) {
            (Some(existing), serde_json::Value::Object(_)) => merge_json_object(existing, value),
            _ => {
                target_obj.insert(key.clone(), value.clone());
            }
        }
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

fn wiki_search_root(
    vault_path: &std::path::Path,
    category: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let wiki_dir = vault_path.join("wiki");
    if let Some(category) = category {
        Ok(wiki_dir.join(validated_wiki_category(category)?))
    } else {
        Ok(wiki_dir)
    }
}

fn validated_wiki_category(category: &str) -> anyhow::Result<&str> {
    match category {
        "projects" | "topics" | "decisions" => Ok(category),
        _ => Err(anyhow::anyhow!(
            "invalid category '{}': must be one of projects, topics, decisions",
            category
        )),
    }
}

fn build_wiki_match(
    vault_path: &std::path::Path,
    path: &std::path::Path,
    content: &str,
    name_match: bool,
    score: f32,
) -> WikiMatch {
    let rel = path
        .strip_prefix(vault_path)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let title = content
        .lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").to_string())
        .unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
    let preview = content.chars().take(500).collect();
    let (created, updated) = extract_wiki_dates(content);

    WikiMatch {
        path: rel,
        title,
        preview,
        name_match,
        created,
        updated,
        score,
    }
}

fn wiki_matches_to_json(matches: Vec<WikiMatch>) -> serde_json::Value {
    let results: Vec<serde_json::Value> = matches
        .into_iter()
        .map(|item| {
            let mut obj = serde_json::json!({
                "path": item.path,
                "title": item.title,
                "preview": item.preview,
                "score": item.score,
            });
            if let Some(created) = item.created {
                obj["created"] = serde_json::Value::String(created);
            }
            if let Some(updated) = item.updated {
                obj["updated"] = serde_json::Value::String(updated);
            }
            obj
        })
        .collect();
    let count = results.len();
    serde_json::json!({ "results": results, "count": count })
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
