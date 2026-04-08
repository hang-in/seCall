use anyhow::Result;
use secall_core::{
    ingest::markdown::{extract_body_text, parse_session_frontmatter},
    search::{tokenizer::create_tokenizer, Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database, SessionRepo},
    vault::{git::VaultGit, Config, Vault},
};

use crate::output::OutputFormat;

use super::ingest::{ingest_sessions, IngestStats};
use super::wiki;

pub async fn run(local_only: bool, dry_run: bool, no_wiki: bool) -> Result<()> {
    let config = Config::load_or_default();
    let vault_git = VaultGit::new(&config.vault.path);

    if dry_run {
        eprintln!("[DRY RUN] No changes will be made.\n");
    }

    // === Phase 0: 이전 sync에서 push되지 않은 변경 자동 커밋 (pull --rebase 실패 방지) ===
    if !local_only && vault_git.is_git_repo() && !dry_run {
        match vault_git.auto_commit() {
            Ok(true) => eprintln!("Auto-committed pending vault changes."),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "auto-commit failed");
                eprintln!("  ⚠ Auto-commit failed: {e}");
            }
        }
    }

    // === Phase 1: Pull (다른 기기 세션 수신) ===
    if !local_only && vault_git.is_git_repo() {
        if dry_run {
            eprintln!("[DRY RUN] Phase 1: Would pull from remote (git pull --rebase origin main)");
        } else {
            eprintln!("Pulling from remote...");
            match vault_git.pull() {
                Ok(result) => {
                    if result.already_up_to_date {
                        eprintln!("  Already up to date.");
                    } else {
                        eprintln!("  <- {} new session files received.", result.new_files);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "git pull failed, continuing with local sync");
                    eprintln!("  Pull failed: {e}");
                }
            }
        }
    }

    // === Phase 2: Reindex (동기화된 MD -> DB) ===
    if dry_run {
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
        eprintln!("[DRY RUN] Phase 2: Would reindex vault ({md_count} MD files found, new ones would be inserted into DB)");
    } else {
        eprintln!("Reindexing vault...");
        let db = Database::open(&get_default_db_path())?;
        let reindex_result = reindex_vault(&config, &db)?;
        eprintln!(
            "  {} new sessions indexed, {} skipped.",
            reindex_result.indexed, reindex_result.skipped
        );

        // === Phase 3: Ingest (로컬 새 세션 -> vault) ===
        eprintln!("Ingesting local sessions...");
        let ingest_result = run_auto_ingest(&config, &db).await?;
        eprintln!(
            "  -> {} ingested, {} skipped, {} errors.",
            ingest_result.ingested, ingest_result.skipped, ingest_result.errors
        );

        // === Phase 3.5: Incremental wiki (새 세션 → wiki 갱신) ===
        if !no_wiki && !ingest_result.new_session_ids.is_empty() {
            if !wiki::command_exists("claude") {
                eprintln!("  ⚠ Claude CLI not found, skipping wiki update.");
            } else {
                let count = ingest_result.new_session_ids.len();
                if count > 10 {
                    eprintln!("  ⚠ {} new sessions — consider running `secall wiki update` in batch mode for efficiency.", count);
                }
                eprintln!("Updating wiki for {} new session(s)...", count);
                for sid in &ingest_result.new_session_ids {
                    match wiki::run_update("sonnet", None, Some(sid.as_str()), false).await {
                        Ok(()) => eprintln!("  ✓ wiki updated for {}", &sid[..sid.len().min(8)]),
                        Err(e) => {
                            eprintln!("  ⚠ wiki failed for {}: {e}", &sid[..sid.len().min(8)])
                        }
                    }
                }
            }
        }

        // === Phase 4: Push (로컬 세션 공유) ===
        if !local_only && vault_git.is_git_repo() {
            eprintln!("Pushing to remote...");
            let hostname = gethostname::gethostname().to_string_lossy().to_string();
            let message = format!(
                "sync: {} new sessions from {}",
                ingest_result.ingested, hostname
            );

            match vault_git.push(&message) {
                Ok(result) => {
                    if result.committed > 0 {
                        eprintln!("  -> {} files pushed.", result.committed);
                    } else {
                        eprintln!("  No changes to push.");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "git push failed");
                    eprintln!("  Push failed: {e}");
                }
            }
        }
    }

    if dry_run {
        if !local_only && vault_git.is_git_repo() {
            eprintln!("[DRY RUN] Phase 3: Would ingest local sessions into vault");
            if !no_wiki {
                eprintln!(
                    "[DRY RUN] Phase 3.5: Would update wiki for new sessions (skip with --no-wiki)"
                );
            }
            eprintln!(
                "[DRY RUN] Phase 4: Would push vault changes to remote (git push origin main)"
            );
        } else {
            eprintln!("[DRY RUN] Phase 3: Would ingest local sessions into vault");
            if !no_wiki {
                eprintln!(
                    "[DRY RUN] Phase 3.5: Would update wiki for new sessions (skip with --no-wiki)"
                );
            }
        }
        eprintln!("\n[DRY RUN] Sync preview complete. No changes made.");
    } else {
        eprintln!("\nSync complete.");
    }
    Ok(())
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
async fn run_auto_ingest(config: &Config, db: &Database) -> Result<IngestStats> {
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
        &OutputFormat::Text,
    )
    .await
}
