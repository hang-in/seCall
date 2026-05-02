use anyhow::Result;
use secall_core::{
    ingest::markdown::{extract_body_text, parse_session_frontmatter},
    jobs::ProgressSink,
    search::{tokenizer::create_tokenizer, Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database, SessionRepo},
    vault::{git::VaultGit, Config, Vault},
};

use crate::output::OutputFormat;

use super::ingest::{ingest_sessions, IngestStats};
use super::wiki;

/// `sync` 명령 인자 — REST DTO/Job 어댑터에서 동일 구조 사용.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SyncArgs {
    pub local_only: bool,
    pub dry_run: bool,
    pub no_wiki: bool,
    pub no_semantic: bool,
    /// graph 증분 갱신 비활성화 (기본: false → 활성화)
    #[serde(default)]
    pub no_graph: bool,
}

/// `sync` 결과 요약 — REST 응답 / SSE Done payload용.
///
/// P33 Task 03(REST 핸들러)에서 직렬화되며, 일부 필드는 CLI 경로에서는
/// 바깥으로 노출되지 않는다.
#[derive(Debug, Default, serde::Serialize)]
pub struct SyncOutcome {
    pub pulled: Option<usize>,
    pub reindexed: usize,
    pub ingested: usize,
    pub wiki_updated: Option<usize>,
    pub pushed: Option<String>,
    /// 부분 실패 시 마지막 phase 에러 메시지. push 실패 등은 fatal로 취급하지 않는다.
    pub partial_failure: Option<String>,
    /// graph 증분 추가된 노드 수 (graph phase 성공 시에만 Some)
    pub graph_nodes_added: Option<usize>,
    /// graph 증분 추가된 엣지 수 (graph phase 성공 시에만 Some)
    pub graph_edges_added: Option<usize>,
}

pub async fn run(
    local_only: bool,
    dry_run: bool,
    no_wiki: bool,
    no_semantic: bool,
    no_graph: bool,
) -> Result<()> {
    let args = SyncArgs {
        local_only,
        dry_run,
        no_wiki,
        no_semantic,
        no_graph,
    };
    let _outcome = run_with_progress(args, &super::NoopSink).await?;
    Ok(())
}

/// Progress 보고가 가능한 sync 본체. CLI는 NoopSink로, REST/Job은 BroadcastSink로 호출한다.
///
/// 기존 `eprintln!` 출력은 모두 그대로 유지되며, phase 경계에 sink 호출만 추가된다.
pub async fn run_with_progress(args: SyncArgs, sink: &dyn ProgressSink) -> Result<SyncOutcome> {
    let SyncArgs {
        local_only,
        dry_run,
        no_wiki,
        no_semantic,
        no_graph,
    } = args;

    let config = Config::load_or_default();
    let vault_git = VaultGit::new(&config.vault.path, &config.vault.branch);

    let mut outcome = SyncOutcome::default();

    // ── init phase ──
    sink.phase_start("init").await;
    if dry_run {
        eprintln!("[DRY RUN] No changes will be made.\n");
        sink.message("[DRY RUN] No changes will be made.").await;
    }

    // === Preflight: vault git 충돌 상태 확인 (Closes #7) ===
    if vault_git.is_git_repo() {
        if let Some(msg) = vault_git.check_conflicted_state() {
            anyhow::bail!("Sync aborted — vault git conflict detected.\n\n{msg}");
        }
    }

    // === Phase 0: 이전 sync에서 push되지 않은 변경 자동 커밋 (pull --rebase 실패 방지) ===
    if vault_git.is_git_repo() && !dry_run {
        match vault_git.auto_commit() {
            Ok(true) => {
                eprintln!("Auto-committed pending vault changes.");
                sink.message("Auto-committed pending vault changes.").await;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "auto-commit failed");
                eprintln!("  ⚠ Auto-commit failed: {e}");
                sink.message(&format!("Auto-commit failed: {e}")).await;
            }
        }
    }
    sink.phase_complete("init", None).await;

    // P36 — cancel check (between init and pull)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (init phase 완료)")
            .await;
        return Ok(outcome);
    }

    // === Phase 1: Pull (다른 기기 세션 수신) ===
    sink.phase_start("pull").await;
    let mut pulled_count: Option<usize> = None;
    if !local_only && vault_git.is_git_repo() {
        if dry_run {
            eprintln!("[DRY RUN] Phase 1: Would pull from remote (git pull --rebase origin main)");
            sink.message("[DRY RUN] Would pull from remote").await;
        } else {
            eprintln!("Pulling from remote...");
            sink.message("Pulling from remote...").await;
            match vault_git.pull() {
                Ok(result) => {
                    if result.already_up_to_date {
                        eprintln!("  Already up to date.");
                        sink.message("Already up to date.").await;
                        pulled_count = Some(0);
                    } else {
                        eprintln!("  <- {} new session files received.", result.new_files);
                        sink.message(&format!("{} new session files received.", result.new_files))
                            .await;
                        pulled_count = Some(result.new_files);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "git pull failed, continuing with local sync");
                    eprintln!("  Pull failed: {e}");
                    sink.message(&format!("Pull failed: {e}")).await;
                    outcome.partial_failure = Some(format!("pull: {e}"));
                }
            }
        }
    }
    outcome.pulled = pulled_count;
    sink.phase_complete("pull", Some(serde_json::json!({ "pulled": pulled_count })))
        .await;

    // P36 — cancel check (between pull and reindex)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (pull phase 완료)")
            .await;
        return Ok(outcome);
    }

    if dry_run {
        // dry-run 경로: 나머지 phase는 안내만 출력하고 종료
        let sessions_dir = config.vault.path.join("raw").join("sessions");
        let md_count = if sessions_dir.exists() {
            walkdir::WalkDir::new(&sessions_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
                .count()
        } else {
            0
        };
        sink.phase_start("reindex").await;
        eprintln!("[DRY RUN] Phase 2: Would reindex vault ({md_count} MD files found, new ones would be inserted into DB)");
        sink.message(&format!(
            "[DRY RUN] Would reindex vault ({md_count} MD files found)"
        ))
        .await;
        sink.phase_complete("reindex", None).await;

        sink.phase_start("ingest").await;
        eprintln!("[DRY RUN] Phase 3: Would ingest local sessions into vault");
        sink.message("[DRY RUN] Would ingest local sessions into vault")
            .await;
        sink.phase_complete("ingest", None).await;

        if !no_wiki {
            sink.phase_start("wiki_update").await;
            eprintln!(
                "[DRY RUN] Phase 3.5: Would update wiki for new sessions (skip with --no-wiki)"
            );
            sink.message("[DRY RUN] Would update wiki for new sessions")
                .await;
            sink.phase_complete("wiki_update", None).await;
        }

        eprintln!("[DRY RUN] Phase 3.7: Would update knowledge graph for new sessions");
        sink.message("[DRY RUN] Would update knowledge graph for new sessions")
            .await;

        if !local_only && vault_git.is_git_repo() {
            sink.phase_start("push").await;
            eprintln!(
                "[DRY RUN] Phase 4: Would push vault changes to remote (git push origin main)"
            );
            sink.message("[DRY RUN] Would push vault changes to remote")
                .await;
            sink.phase_complete("push", None).await;
        }
        eprintln!("\n[DRY RUN] Sync preview complete. No changes made.");
        sink.message("[DRY RUN] Sync preview complete. No changes made.")
            .await;
        return Ok(outcome);
    }

    // === Phase 2: Reindex (동기화된 MD -> DB) ===
    sink.phase_start("reindex").await;
    eprintln!("Reindexing vault...");
    sink.message("Reindexing vault...").await;
    let db = Database::open(&get_default_db_path())?;
    let reindex_result = reindex_vault(&config, &db)?;
    eprintln!(
        "  {} new sessions indexed, {} skipped.",
        reindex_result.indexed, reindex_result.skipped
    );
    sink.message(&format!(
        "{} new sessions indexed, {} skipped.",
        reindex_result.indexed, reindex_result.skipped
    ))
    .await;
    outcome.reindexed = reindex_result.indexed;
    sink.phase_complete(
        "reindex",
        Some(serde_json::json!({
            "indexed": reindex_result.indexed,
            "skipped": reindex_result.skipped,
        })),
    )
    .await;

    // P36 — cancel check (between reindex and ingest)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (reindex phase 완료)")
            .await;
        return Ok(outcome);
    }

    // === Phase 3: Ingest (로컬 새 세션 -> vault) ===
    sink.phase_start("ingest").await;
    eprintln!("Ingesting local sessions...");
    sink.message("Ingesting local sessions...").await;
    let ingest_result = run_auto_ingest(&config, &db, no_semantic, sink).await?;
    eprintln!(
        "  -> {} ingested, {} skipped, {} errors.",
        ingest_result.ingested, ingest_result.skipped, ingest_result.errors
    );
    sink.message(&format!(
        "-> {} ingested, {} skipped, {} errors.",
        ingest_result.ingested, ingest_result.skipped, ingest_result.errors
    ))
    .await;
    outcome.ingested = ingest_result.ingested;
    sink.phase_complete(
        "ingest",
        Some(serde_json::json!({
            "ingested": ingest_result.ingested,
            "skipped": ingest_result.skipped,
            "errors": ingest_result.errors,
        })),
    )
    .await;

    // P36 — cancel check (between ingest and wiki_update)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (ingest phase 완료)")
            .await;
        return Ok(outcome);
    }

    // === Phase 3.5: Incremental wiki (새 세션 → wiki 갱신) ===
    if !no_wiki && !ingest_result.new_session_ids.is_empty() {
        sink.phase_start("wiki_update").await;
        let count = ingest_result.new_session_ids.len();
        if count > 10 {
            eprintln!("  ⚠ {} new sessions — consider running `secall wiki update` in batch mode for efficiency.", count);
            sink.message(&format!(
                "{} new sessions — consider running `secall wiki update` in batch mode.",
                count
            ))
            .await;
        }
        eprintln!("Updating wiki for {} new session(s)...", count);
        sink.message(&format!("Updating wiki for {} new session(s)...", count))
            .await;
        let mut wiki_updated = 0usize;
        let total_wiki = ingest_result.new_session_ids.len();
        for (i, sid) in ingest_result.new_session_ids.iter().enumerate() {
            // P36 — cancel check at top of wiki update loop
            if sink.is_cancelled() {
                sink.message(&format!(
                    "취소 요청 — {}/{} 세션 wiki 갱신 후 종료합니다",
                    i, total_wiki
                ))
                .await;
                outcome.wiki_updated = Some(wiki_updated);
                return Ok(outcome);
            }
            sink.progress((i as f32) / (total_wiki as f32)).await;
            match wiki::run_update(None, None, None, Some(sid.as_str()), false, false, None).await {
                Ok(()) => {
                    eprintln!("  ✓ wiki updated for {}", &sid[..sid.len().min(8)]);
                    sink.message(&format!("wiki updated for {}", &sid[..sid.len().min(8)]))
                        .await;
                    wiki_updated += 1;
                }
                Err(e) => {
                    eprintln!("  ⚠ wiki failed for {}: {e}", &sid[..sid.len().min(8)]);
                    sink.message(&format!(
                        "wiki failed for {}: {e}",
                        &sid[..sid.len().min(8)]
                    ))
                    .await;
                }
            }
        }
        outcome.wiki_updated = Some(wiki_updated);
        sink.phase_complete(
            "wiki_update",
            Some(serde_json::json!({ "wiki_updated": wiki_updated })),
        )
        .await;
    }

    // P36 — cancel check (between wiki_update and graph)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (wiki_update phase 완료)")
            .await;
        return Ok(outcome);
    }

    // === Phase 3.7: Graph 증분 (새 세션 → graph 노드/엣지 추가) ===
    // 본 task(P33-07)는 신규 세션 자체 노드 + 출엣지만 증분 추가한다.
    // cross-session 엣지(same_project, same_day)는 별도로 `secall graph build`를 실행해야 한다.
    if !no_graph && !ingest_result.new_session_ids.is_empty() {
        sink.phase_start("graph").await;
        eprintln!("Updating knowledge graph (incremental)...");
        sink.message("Updating knowledge graph (incremental)...")
            .await;
        match secall_core::graph::extract::extract_for_sessions(
            &db,
            &config.vault.path,
            &ingest_result.new_session_ids,
        ) {
            Ok(report) => {
                eprintln!(
                    "  ✓ graph: {} nodes / {} edges added ({} sessions processed).",
                    report.nodes_added, report.edges_added, report.sessions_processed
                );
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
                eprintln!("  ⚠ Graph incremental failed: {e}");
                sink.message(&format!("Graph incremental failed: {e}"))
                    .await;
                sink.phase_complete("graph", Some(serde_json::json!({ "error": e.to_string() })))
                    .await;
            }
        }
    }

    // P36 — cancel check (between graph and push)
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (graph phase 완료)")
            .await;
        return Ok(outcome);
    }

    // === Phase 4: Push (로컬 세션 공유) ===
    if !local_only && vault_git.is_git_repo() {
        sink.phase_start("push").await;
        eprintln!("Pushing to remote...");
        sink.message("Pushing to remote...").await;
        let hostname = gethostname::gethostname().to_string_lossy().to_string();
        let message = format!(
            "sync: {} new sessions from {}",
            ingest_result.ingested, hostname
        );

        match vault_git.push(&message) {
            Ok(result) => {
                if result.committed > 0 {
                    eprintln!("  -> {} files pushed.", result.committed);
                    sink.message(&format!("-> {} files pushed.", result.committed))
                        .await;
                    outcome.pushed = Some(format!("{} files", result.committed));
                } else {
                    eprintln!("  No changes to push.");
                    sink.message("No changes to push.").await;
                }
                sink.phase_complete(
                    "push",
                    Some(serde_json::json!({ "committed": result.committed })),
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "git push failed");
                eprintln!("  Push failed: {e}");
                sink.message(&format!("Push failed: {e}")).await;
                outcome.partial_failure = Some(format!("push: {e}"));
                sink.phase_complete("push", Some(serde_json::json!({ "error": e.to_string() })))
                    .await;
                // push 실패는 부분 성공으로 처리 — outcome에 기록 후 Ok 반환
            }
        }
    }

    eprintln!("\nSync complete.");
    sink.message("Sync complete.").await;
    Ok(outcome)
}

struct ReindexResult {
    indexed: usize,
    skipped: usize,
}

/// vault/raw/sessions/ 스캔 -> DB에 없는 MD를 인덱싱
fn reindex_vault(config: &Config, db: &Database) -> Result<ReindexResult> {
    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        return Ok(ReindexResult {
            indexed: 0,
            skipped: 0,
        });
    }

    let mut indexed = 0usize;
    let mut skipped = 0usize;

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to read");
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
                continue;
            }
        };

        if fm.session_id.is_empty() {
            continue;
        }

        match db.session_exists(&fm.session_id) {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "DB check failed");
                continue;
            }
        }

        let vault_path = path
            .strip_prefix(&config.vault.path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let body = extract_body_text(&content);

        match db.insert_session_from_vault(&fm, &body, &vault_path) {
            Ok(()) => indexed += 1,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "reindex failed");
            }
        }
    }

    Ok(ReindexResult { indexed, skipped })
}

/// ingest --auto 로직 재사용
///
/// P36 — `sink` 를 ingest_sessions 안쪽 file/embedding 루프 cancel 폴링에 전달.
async fn run_auto_ingest(
    config: &Config,
    db: &Database,
    no_semantic: bool,
    sink: &dyn ProgressSink,
) -> Result<IngestStats> {
    use secall_core::ingest::detect::{
        find_claude_sessions, find_codex_sessions, find_gemini_sessions,
    };

    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow::anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = secall_core::search::vector::create_vector_indexer(config).await;
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    let mut paths = find_claude_sessions(None)?;
    paths.extend(find_codex_sessions(None)?);
    paths.extend(find_gemini_sessions(None)?);

    if paths.is_empty() {
        return Ok(IngestStats {
            ingested: 0,
            skipped: 0,
            errors: 0,
            skipped_min_turns: 0,
            hook_failures: 0,
            new_session_ids: Vec::new(),
            error_details: Vec::new(),
        });
    }

    let vault = Vault::new(config.vault.path.clone());
    vault.init()?;

    ingest_sessions(
        config,
        db,
        paths,
        &engine,
        &vault,
        0,
        false,
        no_semantic,
        &OutputFormat::Text,
        Some(sink),
    )
    .await
}
