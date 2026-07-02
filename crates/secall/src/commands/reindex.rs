use anyhow::Result;
use secall_core::{
    ingest::markdown::{extract_body_text, parse_session_frontmatter, parse_session_turns},
    search::tokenizer::{create_tokenizer, Tokenizer},
    store::{get_default_db_path, Database, SearchRepo, SessionRepo},
    vault::Config,
};

pub fn run(from_vault: bool, repair_missing_turns: bool) -> Result<()> {
    if !from_vault {
        anyhow::bail!("--from-vault flag is required");
    }

    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    let sessions_dir = secall_core::vault::sessions_subdir(&config.vault.path);
    if !sessions_dir.exists() {
        println!("No vault sessions directory found.");
        return Ok(());
    }

    // healing 시에만 tokenizer 준비 (FTS 재삽입용).
    let tokenizer: Option<Box<dyn Tokenizer>> = if repair_missing_turns {
        Some(create_tokenizer(&config.search.tokenizer)?)
    } else {
        None
    };

    let zero_turn_before = db.count_zero_turn_sessions().unwrap_or(0);

    let mut indexed = 0usize;
    let mut skipped = 0usize;
    let mut healed = 0usize;
    let mut errors = 0usize;

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
                errors += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
                errors += 1;
                continue;
            }
        };

        if fm.session_id.is_empty() {
            tracing::warn!(path = %path.display(), "frontmatter missing session_id");
            errors += 1;
            continue;
        }

        let vault_path = path
            .strip_prefix(&config.vault.path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // 중복 체크
        match db.session_exists(&fm.session_id) {
            Ok(true) => {
                // 기존 세션: repair 요청 + turns 0개면 vault 에서 turns 복구 (비파괴 healing).
                if let Some(tok) = tokenizer.as_ref() {
                    match db.count_session_turns(&fm.session_id) {
                        Ok(0) => match heal_session(
                            &db,
                            tok.as_ref(),
                            &fm.session_id,
                            &content,
                            &vault_path,
                        ) {
                            Ok(n) if n > 0 => {
                                healed += 1;
                                tracing::info!(
                                    session = %fm.session_id, turns = n,
                                    "healed zero-turn session from vault"
                                );
                            }
                            Ok(_) => {
                                // 파싱했지만 turn 0개 — 복구 불가로 간주, skip
                                skipped += 1;
                            }
                            Err(e) => {
                                tracing::warn!(session = %fm.session_id, error = %e, "heal failed");
                                errors += 1;
                            }
                        },
                        Ok(_) => {
                            // 정상 turns 존재 → no-op
                            skipped += 1;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "count_turns failed");
                            errors += 1;
                        }
                    }
                } else {
                    skipped += 1;
                }
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "DB check failed");
                errors += 1;
                continue;
            }
        }

        // 신규 세션: 메타데이터 인덱싱.
        let body = extract_body_text(&content);
        match db.insert_session_from_vault(&fm, &body, &vault_path) {
            Ok(()) => {
                indexed += 1;
                // repair 모드: 새 세션도 turns 를 저장해 zero-turn 생성을 막는다.
                // (repair 아니면 기존 동작대로 메타데이터만 — turns 미저장)
                if let Some(tok) = tokenizer.as_ref() {
                    match heal_session(&db, tok.as_ref(), &fm.session_id, &content, &vault_path) {
                        Ok(n) if n > 0 => healed += 1,
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(session = %fm.session_id, error = %e, "heal (new) failed");
                            errors += 1;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "reindex failed");
                errors += 1;
            }
        }
    }

    eprintln!(
        "\nReindex: {} indexed, {} skipped, {} errors",
        indexed, skipped, errors
    );

    if repair_missing_turns {
        let zero_turn_after = db.count_zero_turn_sessions().unwrap_or(0);
        eprintln!(
            "Repair: turns written to {} session(s) from vault. Zero-turn sessions {} -> {} \
             ({} still zero-turn — vault file missing/unresolvable).",
            healed, zero_turn_before, zero_turn_after, zero_turn_after
        );
    }

    Ok(())
}

/// 단일 세션을 vault markdown 에서 복구한다 (비파괴).
///
/// 1. legacy `turn_id=0` 블롭 및 이전 healed FTS 행 제거 (멱등성)
/// 2. `parse_session_turns` 로 복원한 turn 을 `turns` + `turns_fts` 에 저장
/// 3. vault_path 를 실제 파일 경로로 갱신 (status='indexed')
///
/// sessions row(favorite/notes/tags) 와 graph 는 건드리지 않는다.
/// 반환값: 저장한 turn 수.
fn heal_session(
    db: &Database,
    tokenizer: &dyn Tokenizer,
    session_id: &str,
    content: &str,
    vault_rel_path: &str,
) -> Result<usize> {
    let turns = parse_session_turns(content)?;
    if turns.is_empty() {
        return Ok(0);
    }

    // FTS 재삽입 전 기존 행 제거 (legacy blob + 재실행 중복 방지)
    db.clear_session_fts(session_id)?;

    for turn in &turns {
        let tokenized = tokenizer.tokenize_for_fts(&turn.content);
        // insert_turn 은 INSERT OR IGNORE (UNIQUE(session_id, turn_index)) 라 멱등.
        db.insert_turn(session_id, turn)?;
        db.insert_fts(&tokenized, session_id, turn.index)?;
    }

    // vault_path 를 실제 파일 경로로 정정 (legacy 백슬래시/무점 경로 교체).
    db.update_session_vault_path(session_id, vault_rel_path)?;

    Ok(turns.len())
}
