use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use secall_core::{
    hooks::run_post_ingest_hook,
    ingest::{
        detect::{
            detect_parser, find_claude_sessions, find_codex_sessions, find_gemini_sessions,
            find_sessions_for_cwd,
        },
        AgentKind,
    },
    jobs::ProgressSink,
    search::tokenizer::create_tokenizer,
    search::{Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database, SessionRepo},
    vault::{Config, Vault},
};

use crate::output::{print_ingest_result, OutputFormat};

/// `ingest` 명령 인자 — REST DTO/Job 어댑터에서 동일 구조 사용.
///
/// P33 Task 03(REST 핸들러)에서 어댑터를 통해 사용된다.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct IngestArgs {
    pub path: Option<String>,
    pub auto: bool,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub min_turns: usize,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub no_semantic: bool,
    /// ingest 후 신규 세션을 graph에 자동 증분 추가 (기본: false)
    #[serde(default)]
    pub auto_graph: bool,
}

/// `ingest` 결과 요약 — REST 응답 / SSE Done payload용.
#[derive(Debug, Default, serde::Serialize)]
pub struct IngestOutcome {
    pub ingested: usize,
    pub skipped: usize,
    pub errors: usize,
    pub skipped_min_turns: usize,
    pub hook_failures: usize,
    pub new_session_ids: Vec<String>,
    /// auto_graph로 추가된 graph 노드 수 (auto_graph=true 시에만 Some)
    pub graph_nodes_added: Option<usize>,
    /// auto_graph로 추가된 graph 엣지 수 (auto_graph=true 시에만 Some)
    pub graph_edges_added: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
pub struct IngestError {
    pub path: String,
    pub session_id: Option<String>,
    pub phase: IngestPhase,
    pub message: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestPhase {
    Detection,
    Parsing,
    DuplicateCheck,
    VaultWrite,
    Indexing,
}

pub struct IngestStats {
    pub ingested: usize,
    pub skipped: usize,
    pub errors: usize,
    pub skipped_min_turns: usize,
    pub hook_failures: usize,
    pub new_session_ids: Vec<String>,
    pub error_details: Vec<IngestError>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    path: Option<String>,
    auto: bool,
    cwd: Option<PathBuf>,
    min_turns: usize,
    force: bool,
    no_semantic: bool,
    auto_graph: bool,
    format: &OutputFormat,
) -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;
    let vault = Vault::new(config.vault.path.clone());
    vault.init()?;

    // Build search engine (BM25 + optional vector)
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = secall_core::search::vector::create_vector_indexer(&config).await;
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    // Collect paths to ingest
    let paths = collect_paths(path.as_deref(), auto, cwd.as_deref())?;

    if paths.is_empty() {
        println!("No sessions to ingest.");
        return Ok(());
    }

    let stats = ingest_sessions(
        &config,
        &db,
        paths,
        &engine,
        &vault,
        min_turns,
        force,
        no_semantic,
        format,
    )
    .await?;

    match format {
        OutputFormat::Text => {
            if stats.ingested > 0
                || stats.skipped > 0
                || stats.errors > 0
                || stats.skipped_min_turns > 0
            {
                eprintln!(
                    "\nSummary: {} ingested, {} skipped (duplicate), {} errors",
                    stats.ingested, stats.skipped, stats.errors
                );
                if stats.skipped_min_turns > 0 {
                    eprintln!(
                        "         {} skipped (too few turns)",
                        stats.skipped_min_turns
                    );
                }
                if stats.hook_failures > 0 {
                    eprintln!("         {} hook failure(s)", stats.hook_failures);
                }
                if !stats.error_details.is_empty() {
                    eprintln!("\nErrors:");
                    for err in &stats.error_details {
                        let phase = format!("{:?}", err.phase);
                        let loc = err.session_id.as_deref().unwrap_or(&err.path);
                        eprintln!("  [{phase}] {loc} — {}", err.message);
                    }
                }
            }
        }
        OutputFormat::Json => {
            let summary = serde_json::json!({
                "summary": {
                    "ingested": stats.ingested,
                    "skipped": stats.skipped,
                    "errors": stats.errors,
                    "skipped_min_turns": stats.skipped_min_turns,
                },
                "errors": stats.error_details,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).unwrap_or_default()
            );
        }
    }

    if stats.ingested == 0 && stats.errors > 0 {
        return Err(anyhow!("all sessions failed"));
    }

    // auto_graph: 신규 세션을 graph에 증분 추가
    if auto_graph && !stats.new_session_ids.is_empty() {
        match secall_core::graph::extract::extract_for_sessions(
            &db,
            &config.vault.path,
            &stats.new_session_ids,
        ) {
            Ok(report) => {
                eprintln!(
                    "Graph: {} nodes / {} edges added for {} session(s)",
                    report.nodes_added, report.edges_added, report.sessions_processed
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "graph incremental failed");
                eprintln!("Graph incremental failed: {e}");
            }
        }
    }

    Ok(())
}

/// Progress 보고가 가능한 ingest 본체. CLI는 NoopSink, REST/Job은 BroadcastSink로 호출.
///
/// 기존 `run`이 수행하는 print 출력은 IngestOutcome 직렬화에 포함되지 않으므로
/// REST 응답에 필요한 통계만 IngestOutcome으로 반환한다. 내부 호출은 동일한
/// `ingest_sessions`를 사용하므로 핵심 로직 회귀는 발생하지 않는다.
pub async fn run_with_progress(args: IngestArgs, sink: &dyn ProgressSink) -> Result<IngestOutcome> {
    let IngestArgs {
        path,
        auto,
        cwd,
        min_turns,
        force,
        no_semantic,
        auto_graph,
    } = args;

    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;
    let vault = Vault::new(config.vault.path.clone());
    vault.init()?;

    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = secall_core::search::vector::create_vector_indexer(&config).await;
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    // ── detect phase ──
    sink.phase_start("detect").await;
    let paths = collect_paths(path.as_deref(), auto, cwd.as_deref())?;
    sink.message(&format!("Detected {} session file(s).", paths.len()))
        .await;
    sink.phase_complete("detect", Some(serde_json::json!({ "count": paths.len() })))
        .await;

    if paths.is_empty() {
        return Ok(IngestOutcome::default());
    }

    // ── parse_and_insert phase ──
    sink.phase_start("parse_and_insert").await;
    let stats = ingest_sessions(
        &config,
        &db,
        paths,
        &engine,
        &vault,
        min_turns,
        force,
        no_semantic,
        &OutputFormat::Text,
    )
    .await?;
    sink.message(&format!(
        "{} ingested, {} skipped, {} errors.",
        stats.ingested, stats.skipped, stats.errors
    ))
    .await;
    sink.phase_complete(
        "parse_and_insert",
        Some(serde_json::json!({
            "ingested": stats.ingested,
            "skipped": stats.skipped,
            "errors": stats.errors,
        })),
    )
    .await;

    if stats.ingested == 0 && stats.errors > 0 {
        return Err(anyhow!("all sessions failed"));
    }

    let mut outcome = IngestOutcome {
        ingested: stats.ingested,
        skipped: stats.skipped,
        errors: stats.errors,
        skipped_min_turns: stats.skipped_min_turns,
        hook_failures: stats.hook_failures,
        new_session_ids: stats.new_session_ids,
        graph_nodes_added: None,
        graph_edges_added: None,
    };

    // ── graph phase (auto_graph) ──
    if auto_graph && !outcome.new_session_ids.is_empty() {
        sink.phase_start("graph").await;
        match secall_core::graph::extract::extract_for_sessions(
            &db,
            &config.vault.path,
            &outcome.new_session_ids,
        ) {
            Ok(report) => {
                sink.message(&format!(
                    "graph: {} nodes / {} edges added ({} sessions processed).",
                    report.nodes_added, report.edges_added, report.sessions_processed
                ))
                .await;
                outcome.graph_nodes_added = Some(report.nodes_added);
                outcome.graph_edges_added = Some(report.edges_added);
                sink.phase_complete(
                    "graph",
                    Some(serde_json::json!({
                        "nodes_added": report.nodes_added,
                        "edges_added": report.edges_added,
                        "sessions_processed": report.sessions_processed,
                    })),
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "graph incremental failed");
                sink.message(&format!("graph incremental failed: {e}"))
                    .await;
                sink.phase_complete("graph", Some(serde_json::json!({ "error": e.to_string() })))
                    .await;
            }
        }
    }

    Ok(outcome)
}

/// ingest 핵심 로직 — sync.rs에서도 재사용
#[allow(clippy::too_many_arguments)]
pub async fn ingest_sessions(
    config: &Config,
    db: &Database,
    paths: Vec<PathBuf>,
    engine: &SearchEngine,
    vault: &Vault,
    min_turns: usize,
    force: bool,
    no_semantic: bool,
    format: &OutputFormat,
) -> Result<IngestStats> {
    let mut ingested = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;
    let mut skipped_min_turns = 0usize;
    let mut hook_failures = 0usize;
    let mut new_session_ids: Vec<String> = Vec::new();
    let mut error_details: Vec<IngestError> = Vec::new();

    // BM25/vault 완료 후 벡터 임베딩을 일괄 처리하기 위한 수집 목록.
    let mut vector_tasks: Vec<secall_core::ingest::Session> = Vec::new();

    let compiled_rules: Vec<CompiledRule> = {
        let classification = &config.ingest.classification;
        classification
            .rules
            .iter()
            .map(|rule| {
                if let Some(pattern) = &rule.pattern {
                    regex::Regex::new(pattern)
                        .map(|re| CompiledRule::Pattern(re, rule.session_type.clone()))
                        .map_err(|e| anyhow::anyhow!("invalid regex pattern {:?}: {}", pattern, e))
                } else if let Some(project) = &rule.project {
                    Ok(CompiledRule::Project(project.clone(), rule.session_type.clone()))
                } else {
                    Err(anyhow::anyhow!(
                        "classification rule missing both 'pattern' and 'project' fields (session_type: {:?})",
                        rule.session_type
                    ))
                }
            })
            .collect::<anyhow::Result<_>>()?
    };

    for session_path in &paths {
        // detect_parser()를 한 번 호출 — 포맷 탐지와 라우팅을 동시에 결정
        let parser = match detect_parser(session_path) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "failed to detect session format");
                error_details.push(IngestError {
                    path: session_path.display().to_string(),
                    session_id: None,
                    phase: IngestPhase::Detection,
                    message: e.to_string(),
                });
                errors += 1;
                continue;
            }
        };

        // ClaudeAiParser는 항상 parse_all() 경로 (1:N)
        // agent_kind()로 판단하여 포맷·인코딩 방식과 무관하게 정확히 라우팅
        if parser.agent_kind() == AgentKind::ClaudeAi || parser.agent_kind() == AgentKind::ChatGpt {
            match parser.parse_all(session_path) {
                Ok(sessions) => {
                    eprintln!(
                        "Parsed {} conversations from {}",
                        sessions.len(),
                        session_path.display()
                    );
                    for session in sessions {
                        ingest_single_session(
                            config,
                            &compiled_rules,
                            db,
                            engine,
                            vault,
                            session,
                            format,
                            min_turns,
                            force,
                            &mut ingested,
                            &mut skipped,
                            &mut errors,
                            &mut skipped_min_turns,
                            &mut new_session_ids,
                            &mut vector_tasks,
                            &mut error_details,
                            &mut hook_failures,
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(path = %session_path.display(), error = %e, "failed to parse multi-session file");
                    error_details.push(IngestError {
                        path: session_path.display().to_string(),
                        session_id: None,
                        phase: IngestPhase::Parsing,
                        message: e.to_string(),
                    });
                    errors += 1;
                }
            }
            continue;
        }

        // 1:1 파서: filename-stem 힌트로 빠른 중복 체크 (--force 시 스킵)
        if !force {
            let session_id_hint = session_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            match db.session_exists(session_id_hint) {
                Ok(true) => {
                    // 오픈 세션(end_time IS NULL)이면 파일이 변경됐을 수 있으므로 재인제스트
                    match db.is_session_open(session_id_hint) {
                        Ok(true) => {
                            // 기존 레코드 삭제 후 재인제스트
                            if let Err(e) = db.delete_session_full(session_id_hint) {
                                tracing::warn!(
                                    session = session_id_hint,
                                    "failed to delete open session: {}",
                                    e
                                );
                                skipped += 1;
                                continue;
                            }
                            tracing::debug!(session = session_id_hint, "re-ingesting open session");
                        }
                        Ok(false) => {
                            skipped += 1;
                            continue;
                        }
                        Err(e) => {
                            tracing::warn!(session = session_id_hint, "open check failed: {}", e);
                            skipped += 1;
                            continue;
                        }
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::warn!(path = %session_path.display(), error = %e, "DB check failed, skipping");
                    error_details.push(IngestError {
                        path: session_path.display().to_string(),
                        session_id: None,
                        phase: IngestPhase::DuplicateCheck,
                        message: e.to_string(),
                    });
                    errors += 1;
                    continue;
                }
            }
        }

        match parser.parse(session_path) {
            Ok(session) => {
                ingest_single_session(
                    config,
                    &compiled_rules,
                    db,
                    engine,
                    vault,
                    session,
                    format,
                    min_turns,
                    force,
                    &mut ingested,
                    &mut skipped,
                    &mut errors,
                    &mut skipped_min_turns,
                    &mut new_session_ids,
                    &mut vector_tasks,
                    &mut error_details,
                    &mut hook_failures,
                );
            }
            Err(e) => {
                tracing::warn!(path = %session_path.display(), error = %e, "failed to parse session file");
                error_details.push(IngestError {
                    path: session_path.display().to_string(),
                    session_id: None,
                    phase: IngestPhase::Parsing,
                    message: e.to_string(),
                });
                errors += 1;
            }
        }
    }

    // 벡터 인덱싱 일괄 처리 (BM25/vault와 분리하여 체감 속도 개선)
    if !vector_tasks.is_empty() {
        let total = vector_tasks.len();
        eprintln!("Embedding {total} session(s)...");
        let tz = config.timezone();
        for (i, session) in vector_tasks.iter().enumerate() {
            let short = &session.id[..8.min(session.id.len())];
            eprintln!(
                "  [{}/{total}] {short} ({} turns)",
                i + 1,
                session.turns.len()
            );
            if let Err(e) = engine.index_session_vectors(db, session, tz).await {
                tracing::warn!(session = &session.id[..8.min(session.id.len())], error = %e, "vector embedding failed");
                error_details.push(IngestError {
                    path: String::new(),
                    session_id: Some(session.id.clone()),
                    phase: IngestPhase::Indexing,
                    message: e.to_string(),
                });
                errors += 1;
            }
        }
    }

    // 시맨틱 엣지 추출 (graph build 경유 아닌 ingest 직접 연동)
    let semantic_enabled = config.graph.semantic
        && config.graph.semantic_backend != "disabled"
        && config.embedding.backend != "none"
        && !no_semantic
        && !new_session_ids.is_empty();
    if semantic_enabled {
        // 임베딩 모델(bge-m3)이 Ollama에 로드되어 있으면 언로드하여
        // gemma4와 동시 로드로 인한 메모리 압박 방지 (16GB 시스템 대응)
        if config.embedding.backend == "ollama" && config.graph.semantic_backend == "ollama" {
            let embed_model = config.embedding.ollama_model.as_deref().unwrap_or("bge-m3");
            let ollama_url = config
                .embedding
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            let unload_url = format!("{}/api/generate", ollama_url.trim_end_matches('/'));
            let body = serde_json::json!({"model": embed_model, "keep_alive": 0});
            match secall_core::http_post_json(&unload_url, &body).await {
                Ok(_) => tracing::debug!(
                    model = embed_model,
                    "unloaded embedding model before semantic extraction"
                ),
                Err(e) => {
                    tracing::debug!(model = embed_model, "embedding model unload skipped: {}", e)
                }
            }
        }
        eprintln!(
            "Extracting semantic edges for {} session(s)...",
            new_session_ids.len()
        );
        for session_id in &new_session_ids {
            let short = &session_id[..8.min(session_id.len())];
            // vault에서 세션 마크다운 읽기
            let vault_path_opt = match db.get_session_vault_path(session_id) {
                Ok(vp) => vp,
                Err(e) => {
                    tracing::warn!(session = short, "DB error reading vault path: {}", e);
                    continue;
                }
            };
            let md_path = match vault_path_opt {
                Some(vp) => config.vault.path.join(&vp),
                None => {
                    tracing::debug!(
                        session = short,
                        "no vault path, skipping semantic extraction"
                    );
                    continue;
                }
            };
            let content = match std::fs::read_to_string(&md_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(session = short, "failed to read vault file: {}", e);
                    continue;
                }
            };
            let fm = match secall_core::ingest::markdown::parse_session_frontmatter(&content) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(session = short, "failed to parse frontmatter: {}", e);
                    continue;
                }
            };
            let body = secall_core::ingest::markdown::extract_body_text(&content);
            match secall_core::graph::semantic::extract_and_store(db, &config.graph, &fm, &body)
                .await
            {
                Ok(n) => {
                    tracing::debug!(session = short, edges = n, "semantic edges extracted")
                }
                Err(e) => {
                    tracing::warn!(session = short, "semantic extraction skipped: {}", e)
                }
            }
        }
    }

    Ok(IngestStats {
        ingested,
        skipped,
        errors,
        skipped_min_turns,
        hook_failures,
        new_session_ids,
        error_details,
    })
}

/// 분류 규칙 — regex 패턴 또는 project 이름 매칭
pub(crate) enum CompiledRule {
    Pattern(regex::Regex, String),
    Project(String, String),
}

/// 컴파일된 규칙, 첫 번째 user turn 내용, 세션 project로 session_type 결정.
pub(crate) fn apply_classification(
    compiled_rules: &[CompiledRule],
    first_user_content: &str,
    project: Option<&str>,
    default_type: &str,
) -> String {
    if compiled_rules.is_empty() {
        return default_type.to_string();
    }
    compiled_rules
        .iter()
        .find_map(|rule| match rule {
            CompiledRule::Pattern(re, session_type) => {
                if re.is_match(first_user_content) {
                    Some(session_type.clone())
                } else {
                    None
                }
            }
            CompiledRule::Project(proj, session_type) => {
                if project.map(|p| p == proj).unwrap_or(false) {
                    Some(session_type.clone())
                } else {
                    None
                }
            }
        })
        .unwrap_or_else(|| default_type.to_string())
}

/// 단일 Session을 vault + BM25 + 벡터 목록에 ingest
#[allow(clippy::too_many_arguments)]
fn ingest_single_session(
    config: &Config,
    compiled_rules: &[CompiledRule],
    db: &Database,
    engine: &SearchEngine,
    vault: &Vault,
    mut session: secall_core::ingest::Session,
    format: &OutputFormat,
    min_turns: usize,
    force: bool,
    ingested: &mut usize,
    skipped: &mut usize,
    errors: &mut usize,
    skipped_min_turns: &mut usize,
    new_session_ids: &mut Vec<String>,
    vector_tasks: &mut Vec<secall_core::ingest::Session>,
    error_details: &mut Vec<IngestError>,
    hook_failures: &mut usize,
) {
    // 턴 수 필터 — min_turns > 0 이면 짧은 세션 skip
    if min_turns > 0 && session.turns.len() < min_turns {
        *skipped_min_turns += 1;
        return;
    }

    // 세션 분류: 첫 번째 user turn의 내용 또는 project 이름을 규칙과 매칭
    {
        let first_user_content = session
            .turns
            .iter()
            .find(|t| t.role == secall_core::ingest::Role::User)
            .map(|t| t.content.as_str())
            .unwrap_or("");
        session.session_type = apply_classification(
            compiled_rules,
            first_user_content,
            session.project.as_deref(),
            &config.ingest.classification.default,
        );
    }

    // 실제 session.id 기준 중복 체크 (--force 시 기존 데이터 삭제 후 재삽입)
    // compact 후 turn 수가 크게 증가한 경우 자동 재인제스트
    match db.session_exists(&session.id) {
        Ok(true) if !force => {
            // DB turn 수와 파싱된 turn 수 비교 — compact 이후 turn 누락 감지
            let db_turn_count = match db.count_turns_for_session(&session.id) {
                Ok(count) => count,
                Err(e) => {
                    tracing::warn!(session = &session.id, error = %e, "failed to count turns, skipping");
                    *skipped += 1;
                    return;
                }
            };
            if session.turns.len() > db_turn_count + 10 && session.turns.len() > db_turn_count * 2 {
                tracing::info!(
                    session = &session.id,
                    db_turns = db_turn_count,
                    parsed_turns = session.turns.len(),
                    "re-ingesting session with significantly more turns"
                );
                if let Err(e) = db.delete_session_full(&session.id) {
                    tracing::warn!(session = &session.id, error = %e, "failed to delete session for auto re-ingest");
                    *errors += 1;
                    return;
                }
                // 아래로 계속 진행하여 재인제스트
            } else {
                *skipped += 1;
                return;
            }
        }
        Ok(true) => {
            // --force: 기존 세션 데이터 삭제 (turns, vectors 포함)
            if let Err(e) = db.delete_session_full(&session.id) {
                tracing::warn!(session = &session.id, error = %e, "failed to delete existing session for --force");
                error_details.push(IngestError {
                    path: String::new(),
                    session_id: Some(session.id.clone()),
                    phase: IngestPhase::DuplicateCheck,
                    message: e.to_string(),
                });
                *errors += 1;
                return;
            }
            tracing::info!(
                session = &session.id,
                "deleted existing session for re-ingest"
            );
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "DB check failed, skipping");
            error_details.push(IngestError {
                path: String::new(),
                session_id: Some(session.id.clone()),
                phase: IngestPhase::DuplicateCheck,
                message: e.to_string(),
            });
            *errors += 1;
            return;
        }
    }

    // 1. vault 파일 쓰기
    let tz = config.timezone();
    let rel_path = match vault.write_session(&session, tz) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "vault write failed");
            error_details.push(IngestError {
                path: String::new(),
                session_id: Some(session.id.clone()),
                phase: IngestPhase::VaultWrite,
                message: e.to_string(),
            });
            *errors += 1;
            return;
        }
    };

    let vault_path_str = rel_path.to_string_lossy().to_string();

    // 2. BM25 인덱싱 + vault_path 저장 (트랜잭션)
    let bm25_result = db.with_transaction(|| {
        let stats = engine.index_session_bm25(db, &session)?;
        db.update_session_vault_path(&session.id, &vault_path_str)?;
        Ok(stats)
    });

    let index_stats = match bm25_result {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(session = &session.id, error = %e, "indexing failed, rolling back");
            if let Err(rm_err) = std::fs::remove_file(config.vault.path.join(&rel_path)) {
                tracing::warn!(error = %rm_err, "failed to cleanup vault file");
            }
            error_details.push(IngestError {
                path: String::new(),
                session_id: Some(session.id.clone()),
                phase: IngestPhase::Indexing,
                message: e.to_string(),
            });
            *errors += 1;
            return;
        }
    };

    let abs_path = config.vault.path.join(&rel_path);
    print_ingest_result(&session, &abs_path, &index_stats, format);
    *ingested += 1;
    new_session_ids.push(session.id.clone());

    if let Err(e) = run_post_ingest_hook(config, &session, &abs_path, tz) {
        tracing::warn!(session = &session.id[..8.min(session.id.len())], error = %e, "post-ingest hook failed");
        *hook_failures += 1;
    }

    // 3. 벡터 임베딩을 위해 수집 (skip_embed_types에 포함된 session_type은 제외)
    let skip_embed = config
        .ingest
        .classification
        .skip_embed_types
        .contains(&session.session_type);
    if !skip_embed {
        vector_tasks.push(session);
    }
}

fn collect_paths(path: Option<&str>, auto: bool, cwd: Option<&Path>) -> Result<Vec<PathBuf>> {
    if auto {
        if let Some(cwd) = cwd {
            find_sessions_for_cwd(cwd)
        } else {
            // Collect sessions from all supported agents
            let mut paths = find_claude_sessions(None)?;
            paths.extend(find_codex_sessions(None)?);
            paths.extend(find_gemini_sessions(None)?);
            Ok(paths)
        }
    } else if let Some(p) = path {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            Ok(vec![pb])
        } else if pb.is_dir() {
            let mut paths = find_claude_sessions(Some(&pb))?;
            paths.extend(find_codex_sessions(Some(&pb))?);
            paths.extend(find_gemini_sessions(Some(&pb))?);
            Ok(paths)
        } else if pb.is_absolute() || p.contains('/') || pb.extension().is_some() {
            // 경로 구문을 가지지만 존재하지 않는 경우 → 그대로 전달해 Detection 단계에서 에러 리포트 생성
            Ok(vec![pb])
        } else {
            // 확장자/슬래시 없는 짧은 문자열 → 세션 ID로 조회
            find_session_by_id(p)
        }
    } else {
        Err(anyhow!("Provide a path, session ID, or use --auto"))
    }
}

fn find_session_by_id(id: &str) -> Result<Vec<PathBuf>> {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects");

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(&base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            if stem == id
                || stem.starts_with(&format!("{id}_"))
                || stem.starts_with(&format!("{id}-"))
            {
                found.push(p.to_path_buf());
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    fn pattern_rules(patterns: &[(&str, &str)]) -> Vec<CompiledRule> {
        patterns
            .iter()
            .map(|(p, t)| CompiledRule::Pattern(Regex::new(p).unwrap(), t.to_string()))
            .collect()
    }

    fn project_rules(projects: &[(&str, &str)]) -> Vec<CompiledRule> {
        projects
            .iter()
            .map(|(p, t)| CompiledRule::Project(p.to_string(), t.to_string()))
            .collect()
    }

    #[test]
    fn test_matches_first_rule() {
        let r = pattern_rules(&[("^\\[자동화\\]", "automated")]);
        assert_eq!(
            apply_classification(&r, "[자동화] 월간 보고", None, "interactive"),
            "automated"
        );
    }

    #[test]
    fn test_matches_second_rule() {
        let r = pattern_rules(&[("^\\[자동화\\]", "automated"), ("^# Wiki", "automated")]);
        assert_eq!(
            apply_classification(&r, "# Wiki Update", None, "interactive"),
            "automated"
        );
    }

    #[test]
    fn test_no_match_uses_default() {
        let r = pattern_rules(&[("^\\[자동화\\]", "automated")]);
        assert_eq!(
            apply_classification(&r, "일반 질문입니다", None, "interactive"),
            "interactive"
        );
    }

    #[test]
    fn test_empty_rules_returns_default() {
        assert_eq!(
            apply_classification(&[], "아무 내용", None, "interactive"),
            "interactive"
        );
    }

    #[test]
    fn test_empty_content() {
        let r = pattern_rules(&[("^\\[자동화\\]", "automated")]);
        assert_eq!(
            apply_classification(&r, "", None, "interactive"),
            "interactive"
        );
    }

    #[test]
    fn test_first_match_wins() {
        let r = pattern_rules(&[("test", "type-a"), ("test", "type-b")]);
        assert_eq!(
            apply_classification(&r, "test content", None, "default"),
            "type-a"
        );
    }

    #[test]
    fn test_project_rule_matches() {
        let r = project_rules(&[("macbook", "automated")]);
        assert_eq!(
            apply_classification(&r, "아무 내용", Some("macbook"), "interactive"),
            "automated"
        );
    }

    #[test]
    fn test_project_rule_no_match() {
        let r = project_rules(&[("macbook", "automated")]);
        assert_eq!(
            apply_classification(&r, "아무 내용", Some("otherproject"), "interactive"),
            "interactive"
        );
    }

    #[test]
    fn test_project_rule_no_project_field() {
        let r = project_rules(&[("macbook", "automated")]);
        assert_eq!(
            apply_classification(&r, "아무 내용", None, "interactive"),
            "interactive"
        );
    }

    #[test]
    fn test_mixed_rules_project_wins_first() {
        let r = vec![
            CompiledRule::Project("macbook".to_string(), "automated".to_string()),
            CompiledRule::Pattern(
                Regex::new("^\\[자동화\\]").unwrap(),
                "automated".to_string(),
            ),
        ];
        // project 규칙이 앞에 있으므로 macbook 프로젝트는 project 규칙으로 먼저 매칭
        assert_eq!(
            apply_classification(&r, "일반 내용", Some("macbook"), "interactive"),
            "automated"
        );
    }
}
