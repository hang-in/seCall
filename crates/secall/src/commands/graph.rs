use anyhow::Result;
use secall_core::{
    graph::{build::build_graph, export::export_graph_json, semantic::extract_and_store},
    ingest::markdown::{extract_body_text, parse_session_frontmatter},
    jobs::ProgressSink,
    store::{get_default_db_path, session_repo::GraphRebuildFilter, Database},
    vault::Config,
};

use super::ingest::{
    extract_one_session_semantic, unload_embedding_model_if_needed, ExtractOneResult,
};
use super::NoopSink;

/// 전체 세션에 대해 시맨틱 엣지만 재추출. 임베딩은 건드리지 않음.
pub async fn run_semantic(
    delay_secs: f64,
    limit: Option<usize>,
    backend: Option<String>,
    api_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<()> {
    let mut config = Config::load_or_default();

    // CLI 플래그 오버라이드 (우선순위: CLI > 환경변수 > config.toml > 기본값)
    if let Some(b) = backend {
        config.graph.semantic_backend = b;
    }
    if let Some(u) = api_url {
        config.graph.ollama_url = Some(u);
    }
    if let Some(m) = model {
        match config.graph.semantic_backend.as_str() {
            "gemini" => config.graph.gemini_model = Some(m),
            "anthropic" => config.graph.anthropic_model = Some(m),
            _ => config.graph.ollama_model = Some(m),
        }
    }
    if let Some(k) = api_key {
        config.graph.gemini_api_key = Some(k);
    }
    let db = Database::open(&get_default_db_path())?;

    if !config.graph.semantic {
        eprintln!("Semantic extraction is disabled (graph.semantic = false in config).");
        return Ok(());
    }
    if config.graph.semantic_backend == "disabled" {
        eprintln!(
            "Semantic backend is 'disabled'. Set graph.semantic_backend = \"ollama\" in config."
        );
        return Ok(());
    }

    // 임베딩 모델 언로드 (gemma4와 동시 로드 방지)
    if config.embedding.backend == "ollama" {
        let embed_model = config.embedding.ollama_model.as_deref().unwrap_or("bge-m3");
        let ollama_url = config
            .embedding
            .ollama_url
            .as_deref()
            .unwrap_or("http://localhost:11434");
        let unload_url = format!("{}/api/generate", ollama_url.trim_end_matches('/'));
        let _ = secall_core::http_post_json(
            &unload_url,
            &serde_json::json!({"model": embed_model, "keep_alive": 0}),
        )
        .await;
    }

    // vault_path가 있는 세션만 추출
    let all_sessions: Vec<(String, String)> = db
        .list_session_vault_paths()?
        .into_iter()
        .filter_map(|(id, vp)| vp.map(|p| (id, p)))
        .collect();
    let total = all_sessions.len();
    let sessions: Vec<_> = match limit {
        Some(n) => all_sessions.into_iter().take(n).collect(),
        None => all_sessions,
    };
    let process_count = sessions.len();

    eprintln!(
        "Extracting semantic edges for {process_count}/{total} sessions (backend: {})...",
        config.graph.semantic_backend
    );

    let mut ok = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (i, (session_id, vault_path)) in sessions.iter().enumerate() {
        let short = &session_id[..8.min(session_id.len())];
        let md_path = config.vault.path.join(vault_path);

        let content = match std::fs::read_to_string(&md_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(session = short, "cannot read vault file: {}", e);
                skipped += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(session = short, "cannot parse frontmatter: {}", e);
                skipped += 1;
                continue;
            }
        };

        let body = extract_body_text(&content);
        match extract_and_store(&db, &config.graph, &fm, &body).await {
            Ok(n) => {
                eprintln!("  [{}/{}] {} — {} edges", i + 1, process_count, short, n);
                ok += 1;
            }
            Err(e) => {
                eprintln!("  [{}/{}] {} — FAILED: {}", i + 1, process_count, short, e);
                failed += 1;
            }
        }

        if delay_secs > 0.0 && i + 1 < process_count {
            tokio::time::sleep(std::time::Duration::from_secs_f64(delay_secs)).await;
        }
    }

    eprintln!("\nDone: {} ok, {} skipped, {} failed", ok, skipped, failed);
    Ok(())
}

pub fn run_build(since: Option<&str>, force: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    if force {
        eprintln!("Clearing existing graph...");
    }
    eprintln!("Building knowledge graph...");

    let result = build_graph(&db, &config.vault.path, since, force)?;

    eprintln!(
        "  {} sessions processed, {} skipped, {} failed.",
        result.sessions_processed, result.sessions_skipped, result.sessions_failed
    );
    eprintln!(
        "  {} nodes, {} edges created.",
        result.nodes_created, result.edges_created
    );
    Ok(())
}

pub fn run_stats() -> Result<()> {
    let db = Database::open(&get_default_db_path())?;
    let stats = db.graph_stats()?;

    println!("Graph Statistics:");
    println!("  Nodes: {}", stats.node_count);
    println!("  Edges: {}", stats.edge_count);
    println!();

    println!("Nodes by type:");
    for (t, c) in &stats.nodes_by_type {
        println!("  {}: {}", t, c);
    }
    println!();

    println!("Edges by relation:");
    for (r, c) in &stats.edges_by_relation {
        println!("  {}: {}", r, c);
    }
    Ok(())
}

pub fn run_export() -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    let graph_dir = config.vault.path.join("graph");
    std::fs::create_dir_all(&graph_dir)?;

    let output_path = graph_dir.join("graph.json");
    export_graph_json(&db, &output_path)?;

    eprintln!("Exported to {}", output_path.display());
    Ok(())
}

// ─── P37 Task 01: graph rebuild ─────────────────────────────────────────────

/// `graph rebuild` 명령 인자 — REST DTO/Job 어댑터(Task 02)에서 동일 구조 사용.
///
/// 우선순위는 `GraphRebuildFilter` 와 동일: `session` > `all` > `retry_failed` > `since`.
/// 모든 필드 비활성이면 빈 결과 반환.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct GraphRebuildArgs {
    pub since: Option<String>,
    pub session: Option<String>,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub retry_failed: bool,
}

impl From<GraphRebuildArgs> for GraphRebuildFilter {
    fn from(args: GraphRebuildArgs) -> Self {
        GraphRebuildFilter {
            since: args.since,
            session: args.session,
            all: args.all,
            retry_failed: args.retry_failed,
        }
    }
}

/// `graph rebuild` 결과 요약 — REST 응답 / SSE Done payload 용.
#[derive(Debug, Default, serde::Serialize)]
pub struct GraphRebuildOutcome {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub edges_added: usize,
}

/// Progress 보고가 가능한 graph rebuild 본체.
///
/// CLI 는 `NoopSink`, REST/Job 은 `BroadcastSink` 로 호출.
/// P36 cancel 패턴: 매 세션 시작 지점에서 `is_cancelled()` 폴링 → 부분 outcome 으로 early return.
pub async fn run_rebuild(
    args: GraphRebuildArgs,
    sink: &dyn ProgressSink,
) -> Result<GraphRebuildOutcome> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    // 1. 처리 대상 ID 목록
    let filter: GraphRebuildFilter = args.into();
    let ids = db.list_sessions_for_graph_rebuild(filter)?;

    if ids.is_empty() {
        sink.message("처리할 세션 없음").await;
        return Ok(GraphRebuildOutcome::default());
    }

    // 2. 임베딩 모델 unload — 시맨틱 추출 진입 시점에 한 번만
    unload_embedding_model_if_needed(&config).await;

    let total = ids.len();
    let mut outcome = GraphRebuildOutcome::default();

    sink.message(&format!("Graph rebuild: {} session(s) to process", total))
        .await;

    let now_secs = chrono::Utc::now().timestamp();

    for (i, id) in ids.iter().enumerate() {
        // 3. 안전 지점 cancel 폴링 (P36 패턴)
        if sink.is_cancelled() {
            sink.message(&format!("취소 요청 — {}/{} 처리 후 종료합니다", i, total))
                .await;
            return Ok(outcome);
        }
        if total > 0 {
            sink.progress((i as f32) / (total as f32)).await;
        }

        // 4. 단일 세션 시맨틱 추출 (ingest 와 공유)
        match extract_one_session_semantic(&db, &config, id).await {
            ExtractOneResult::Extracted(n) => {
                outcome.succeeded += 1;
                outcome.edges_added += n;
                // 성공 시에만 timestamp 갱신
                if let Err(e) = db.update_semantic_extracted_at(id, now_secs) {
                    let short = &id[..8.min(id.len())];
                    tracing::warn!(
                        session = short,
                        error = %e,
                        "failed to update semantic_extracted_at"
                    );
                }
            }
            ExtractOneResult::Skipped(_reason) => {
                outcome.skipped += 1;
            }
            ExtractOneResult::Failed(_e) => {
                outcome.failed += 1;
            }
        }
        outcome.processed += 1;
    }

    sink.message(&format!(
        "완료: succeeded={}, failed={}, skipped={}, edges_added={}",
        outcome.succeeded, outcome.failed, outcome.skipped, outcome.edges_added
    ))
    .await;

    Ok(outcome)
}

/// CLI wrapper — `NoopSink` 사용 (P36 wiki `run_update` 패턴).
pub async fn run_rebuild_cli(args: GraphRebuildArgs) -> Result<()> {
    let outcome = run_rebuild(args, &NoopSink).await?;
    eprintln!(
        "Graph rebuild complete: processed={}, succeeded={}, failed={}, skipped={}, edges_added={}",
        outcome.processed, outcome.succeeded, outcome.failed, outcome.skipped, outcome.edges_added,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `run_rebuild` 는 `SECALL_DB_PATH` 환경변수를 통해 DB 를 연다.
    /// 테스트들이 병렬 실행될 때 같은 환경변수를 동시에 set/unset 하면 race 가 발생하므로
    /// 환경변수 변경을 직렬화한다.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// vault_path 가 설정되지 않은(또는 vault 가 비어있는) 세션은
    /// `extract_one_session_semantic` 이 Skipped 로 판정한다.
    /// 본 테스트들은 filter 가 정확한 ID 세트를 선택하는지(processed 값)만 검증.
    fn insert_minimal_session(db: &Database, id: &str) {
        use chrono::Utc;
        use secall_core::ingest::{AgentKind, Session, TokenUsage};

        let session = Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: None,
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc::now(),
            end_time: None,
            turns: Vec::new(),
            total_tokens: TokenUsage::default(),
            session_type: "interactive".to_string(),
        };
        use secall_core::store::SessionRepo;
        db.insert_session(&session).unwrap();
    }

    #[tokio::test]
    async fn test_run_rebuild_retry_failed_only_processes_null_sessions() {
        let _env_guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.sqlite");
        std::env::set_var("SECALL_DB_PATH", &path);

        let db = Database::open(&get_default_db_path()).unwrap();

        insert_minimal_session(&db, "rf-null-1");
        insert_minimal_session(&db, "rf-null-2");
        insert_minimal_session(&db, "rf-done");
        // rf-done 만 추출 완료 처리 → retry_failed 대상에서 제외
        db.update_semantic_extracted_at("rf-done", 999).unwrap();
        drop(db);

        let args = GraphRebuildArgs {
            retry_failed: true,
            ..Default::default()
        };
        let outcome = run_rebuild(args, &NoopSink).await.unwrap();

        // 두 NULL 세션만 처리됨 (vault 누락이라 모두 skipped 로 카운트되지만
        // processed 는 NULL 세션 수와 정확히 일치)
        assert_eq!(outcome.processed, 2);
        assert_eq!(outcome.skipped, 2);
        assert_eq!(outcome.succeeded, 0);
        assert_eq!(outcome.failed, 0);

        std::env::remove_var("SECALL_DB_PATH");
    }

    #[tokio::test]
    async fn test_run_rebuild_session_filter_processes_one() {
        let _env_guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.sqlite");
        std::env::set_var("SECALL_DB_PATH", &path);

        let db = Database::open(&get_default_db_path()).unwrap();

        insert_minimal_session(&db, "sf-1");
        insert_minimal_session(&db, "sf-2");
        insert_minimal_session(&db, "sf-3");
        drop(db);

        let args = GraphRebuildArgs {
            session: Some("sf-2".to_string()),
            ..Default::default()
        };
        let outcome = run_rebuild(args, &NoopSink).await.unwrap();

        assert_eq!(outcome.processed, 1);

        std::env::remove_var("SECALL_DB_PATH");
    }
}
