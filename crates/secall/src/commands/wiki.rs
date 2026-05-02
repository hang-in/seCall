use std::path::PathBuf;

use anyhow::Result;
use secall_core::{
    jobs::ProgressSink,
    store::{get_default_db_path, Database},
    vault::Config,
};

/// `wiki update` 명령 인자 — REST DTO/Job 어댑터에서 동일 구조 사용.
///
/// P33 Task 03(REST 핸들러)에서 어댑터를 통해 사용된다.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WikiUpdateArgs {
    pub model: Option<String>,
    pub backend: Option<String>,
    pub since: Option<String>,
    pub session: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub review: bool,
    pub review_model: Option<String>,
}

/// `wiki update` 결과 요약 — REST 응답 / SSE Done payload용.
///
/// 상세 페이지 별 결과는 stdout/stderr로 전달되고, 본 구조체에는
/// 호출자가 후속 작업에 사용할 수 있는 통계만 담는다.
#[derive(Debug, Default, serde::Serialize)]
pub struct WikiOutcome {
    pub backend: String,
    pub target: String,
    /// 작성된 위키 페이지 개수 (0 이상). 비-haiku 백엔드는 항상 0.
    pub pages_written: usize,
}

/// Progress 보고가 가능한 wiki update 본체.
///
/// 기존 `run_update`는 NoopSink wrapper로 호출되며 출력은 전부 보존된다.
/// 본 함수는 phase 경계만 sink로 보고한다.
pub async fn run_with_progress(
    args: WikiUpdateArgs,
    sink: &dyn ProgressSink,
) -> Result<WikiOutcome> {
    let backend_label = args
        .backend
        .clone()
        .unwrap_or_else(|| "(default)".to_string());
    let target_label = args
        .session
        .as_deref()
        .map(|s| format!("session:{}", &s[..s.len().min(8)]))
        .unwrap_or_else(|| "all sessions".to_string());

    sink.phase_start("prompt_build").await;
    sink.message(&format!(
        "Preparing wiki update (backend={}, target={})",
        backend_label, target_label
    ))
    .await;
    sink.phase_complete("prompt_build", None).await;

    sink.phase_start("llm_call").await;
    sink.message("Generating wiki content...").await;
    // P36 — 내부 session/page loop + LLM 호출 직전 cancel 폴링을 위해 sink 전달
    if sink.is_cancelled() {
        sink.message("취소 요청 — 부분 결과로 종료합니다 (prompt_build phase 완료)")
            .await;
        return Ok(WikiOutcome {
            backend: backend_label,
            target: target_label,
            pages_written: 0,
        });
    }
    // run_update가 prompt build → llm call → lint → merge → write를 모두 처리.
    // Phase 세분화는 run_update 내부 리팩토링이 필요하나 본 task 범위 밖.
    let outcome = run_update_with_sink(
        args.model.as_deref(),
        args.backend.as_deref(),
        args.since.as_deref(),
        args.session.as_deref(),
        args.dry_run,
        args.review,
        args.review_model.as_deref(),
        Some(sink),
    )
    .await;
    sink.phase_complete("llm_call", None).await;

    sink.phase_start("lint").await;
    sink.phase_complete("lint", None).await;

    sink.phase_start("merge_and_write").await;
    let result = match outcome {
        Ok(pages_written) => {
            sink.message("Wiki update complete.").await;
            sink.phase_complete("merge_and_write", None).await;
            // P36 rework — run_update_with_sink 가 반환한 카운트 그대로 outcome 에 반영.
            // 정상 완료든 cancel 시점 부분 완료든 동일 경로.
            Ok(WikiOutcome {
                backend: backend_label,
                target: target_label,
                pages_written,
            })
        }
        Err(e) => {
            sink.message(&format!("Wiki update failed: {e}")).await;
            sink.phase_complete(
                "merge_and_write",
                Some(serde_json::json!({ "error": e.to_string() })),
            )
            .await;
            Err(e)
        }
    };
    result
}

pub async fn run_update(
    model: Option<&str>,
    backend: Option<&str>,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
    review: bool,
    review_model: Option<&str>,
) -> Result<()> {
    // P36 rework — run_update_with_sink 가 page count 반환하지만 CLI 경로에서는 무시.
    run_update_with_sink(
        model,
        backend,
        since,
        session,
        dry_run,
        review,
        review_model,
        None,
    )
    .await
    .map(|_| ())
}

/// P36 — `run_update` 의 sink-aware 버전. 내부 session/page 루프와 LLM 호출
/// 직전에 cancel 폴링하기 위해 옵셔널 sink 를 받는다.
///
/// P36 rework — `run_with_progress` 가 outcome 에 정확한 페이지 수를 반영하도록
/// **새로 작성된(또는 덮어쓴 첫 작성)** 페이지 카운트를 반환한다.
/// review-regen 은 같은 페이지 덮어쓰기 → 카운트 증가 안 함.
/// 비-haiku 백엔드는 stdout 출력만 → 카운트 0.
#[allow(clippy::too_many_arguments)]
async fn run_update_with_sink(
    model: Option<&str>,
    backend: Option<&str>,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
    review: bool,
    review_model: Option<&str>,
    sink: Option<&dyn ProgressSink>,
) -> Result<usize> {
    // P36 rework — 작성된 페이지 누적. 정상 완료/취소 모두 같은 변수 사용.
    let mut pages_written: usize = 0;

    // 1. wiki/ directory check
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        anyhow::bail!("wiki/ directory not found. Run `secall init` first.");
    }

    // 4. 백엔드 선택: --backend 플래그 → config wiki.default_backend → "claude"
    let backend_name = backend
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.wiki.default_backend.clone());
    let resolved_model = resolve_backend_model(&config, &backend_name, model);

    // 2. Load prompt — haiku 백엔드는 세션 데이터를 직접 주입
    let prompt = if backend_name == "haiku" {
        build_haiku_prompt(&config, &wiki_dir, session, since)?
    } else if let Some(sid) = session {
        load_incremental_prompt(sid)?
    } else {
        load_batch_prompt(since)?
    };

    // 3. dry-run: print prompt and exit
    if dry_run {
        println!("{prompt}");
        return Ok(pages_written);
    }

    let target = if let Some(sid) = session {
        format!("session {}", &sid[..sid.len().min(8)])
    } else {
        "all sessions".to_string()
    };
    eprintln!("Wiki update: {} (backend: {})", target, backend_name);

    // 5. WikiBackend 인스턴스 생성
    let backend_box: Box<dyn secall_core::wiki::WikiBackend> = match backend_name.as_str() {
        "haiku" => {
            let cfg = config.wiki_backend_config("haiku");
            let system_prompt = load_haiku_system_prompt();
            Box::new(secall_core::wiki::HaikuBackend::from_env(
                cfg.model,
                cfg.max_tokens,
                system_prompt,
            )?)
        }
        "ollama" => {
            let cfg = config.wiki_backend_config("ollama");
            Box::new(secall_core::wiki::OllamaBackend {
                api_url: cfg
                    .api_url
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                model: cfg.model.unwrap_or_else(|| "llama3".to_string()),
                max_tokens: cfg.max_tokens,
            })
        }
        "lmstudio" => {
            let cfg = config.wiki_backend_config("lmstudio");
            Box::new(secall_core::wiki::LmStudioBackend {
                api_url: cfg
                    .api_url
                    .unwrap_or_else(|| "http://localhost:1234".to_string()),
                model: cfg.model.unwrap_or_else(|| "local-model".to_string()),
                max_tokens: cfg.max_tokens,
            })
        }
        "codex" => Box::new(secall_core::wiki::CodexBackend {
            model: resolved_model.clone(),
            vault_path: config.vault.path.clone(),
        }),
        "claude" => Box::new(secall_core::wiki::ClaudeBackend {
            model: resolved_model.clone(),
            vault_path: config.vault.path.clone(),
        }),
        _ => {
            anyhow::bail!(
                "Unknown backend '{}'. Supported: claude, codex, haiku, ollama, lmstudio",
                backend_name
            );
        }
    };

    // 6. 생성 + 후처리
    if backend_name == "haiku" && session.is_none() {
        // ── 배치 모드: 프로젝트별 개별 호출 ──
        let db = Database::open(&get_default_db_path())?;
        let since_date = since.unwrap_or("2000-01-01");
        let sessions = db.get_sessions_since(since_date)?;
        if sessions.is_empty() {
            eprintln!("  No sessions found since {}", since_date);
            return Ok(pages_written);
        }

        let mut by_project: std::collections::BTreeMap<
            String,
            Vec<&secall_core::store::db::SessionMeta>,
        > = std::collections::BTreeMap::new();
        for s in &sessions {
            let proj = s.project.as_deref().unwrap_or("(기타)").to_string();
            by_project.entry(proj).or_default().push(s);
        }

        let resolved_model = resolve_review_model(review_model, &config);

        let total_proj = by_project.len();
        for (proj_idx, (proj_name, proj_sessions)) in by_project.iter().enumerate() {
            // P36 — cancel check at top of project loop
            if let Some(s) = sink {
                if s.is_cancelled() {
                    s.message(&format!(
                        "취소 요청 — {}/{} 프로젝트까지 처리 후 종료합니다",
                        proj_idx, total_proj
                    ))
                    .await;
                    return Ok(pages_written);
                }
                if total_proj > 0 {
                    s.progress((proj_idx as f32) / (total_proj as f32)).await;
                }
            }
            let session_ids: Vec<String> = proj_sessions.iter().map(|s| s.id.clone()).collect();
            let vault_paths = collect_vault_paths(&db, &session_ids);
            let proj_prompt = build_haiku_single_project_prompt(&db, proj_name, proj_sessions)?;

            eprintln!("  Generating wiki for project: {}...", proj_name);
            // P36 — cancel check just before LLM call (expensive)
            if let Some(s) = sink {
                if s.is_cancelled() {
                    s.message(&format!(
                        "취소 요청 — LLM 호출 직전 취소 ({} 프로젝트)",
                        proj_name
                    ))
                    .await;
                    return Ok(pages_written);
                }
            }
            let output = backend_box.generate(&proj_prompt).await?;

            if output.trim().is_empty() {
                eprintln!("    (no output, skipping)");
                continue;
            }

            let page_path = format!("projects/{}.md", safe_project_name(proj_name));

            let validated = secall_core::wiki::lint::validate_frontmatter(&output, &session_ids);
            let merged = secall_core::wiki::lint::merge_with_existing(
                &wiki_dir,
                &page_path,
                &validated,
                &session_ids,
            )?;
            let wiki_pages = collect_wiki_pages(&wiki_dir);
            let linked = secall_core::wiki::lint::insert_obsidian_links(
                &merged,
                &session_ids,
                &vault_paths,
                &wiki_pages,
            );

            let full_path = wiki_dir.join(&page_path);
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full_path, &linked)?;
            // P36 rework — 새 페이지 작성 성공 시 카운트 +1.
            // (review-regen 은 같은 파일 덮어쓰기라 카운트 증가 안 함)
            pages_written += 1;
            eprintln!("    Written: {}", full_path.display());

            match secall_core::wiki::lint::run_markdownlint(&full_path) {
                Ok(Some(msg)) => eprintln!("    Lint: {}", msg),
                Ok(None) => {}
                Err(e) => eprintln!("    Lint error (skipped): {}", e),
            }

            if review {
                // markdownlint가 파일을 수정했을 수 있으므로 최종 저장본을 다시 읽음
                let final_content =
                    std::fs::read_to_string(&full_path).unwrap_or_else(|_| linked.clone());
                let source_summary = build_review_source(&db, &session_ids);
                let needs_regen =
                    run_review(&resolved_model, &final_content, &source_summary).await;

                // error급 이슈 → 1회 재생성 후 재검수 (무한 루프 방지: 최대 1회)
                if needs_regen {
                    // P36 — cancel check before regeneration LLM call
                    if let Some(s) = sink {
                        if s.is_cancelled() {
                            s.message(&format!(
                                "취소 요청 — 재생성 직전 취소 ({} 프로젝트)",
                                proj_name
                            ))
                            .await;
                            return Ok(pages_written);
                        }
                    }
                    eprintln!("    Regenerating due to review errors...");
                    match backend_box.generate(&proj_prompt).await {
                        Ok(regen_output) if !regen_output.trim().is_empty() => {
                            let validated2 = secall_core::wiki::lint::validate_frontmatter(
                                &regen_output,
                                &session_ids,
                            );
                            let merged2 = secall_core::wiki::lint::merge_with_existing(
                                &wiki_dir,
                                &page_path,
                                &validated2,
                                &session_ids,
                            )
                            .unwrap_or(validated2);
                            let wiki_pages2 = collect_wiki_pages(&wiki_dir);
                            let linked2 = secall_core::wiki::lint::insert_obsidian_links(
                                &merged2,
                                &session_ids,
                                &vault_paths,
                                &wiki_pages2,
                            );
                            if let Err(e) = std::fs::write(&full_path, &linked2) {
                                eprintln!("    Write failed, skipping re-review: {e}");
                            } else {
                                // 재검수 (반환값 무시 — 재시도는 1회만)
                                run_review(&resolved_model, &linked2, &source_summary).await;
                            }
                        }
                        _ => eprintln!("    Regeneration skipped (empty output)"),
                    }
                }
            }
        }
        eprintln!(
            "  ✓ Wiki batch update complete ({} projects).",
            by_project.len()
        );
    } else if backend_name == "haiku" {
        // ── 인크리멘탈 모드: 단일 세션 ──
        // P36 — cancel check just before LLM call
        if let Some(s) = sink {
            if s.is_cancelled() {
                s.message("취소 요청 — LLM 호출 직전 취소 (haiku incremental)")
                    .await;
                return Ok(pages_written);
            }
        }
        eprintln!("  Launching {}...", backend_box.name());
        let output = backend_box.generate(&prompt).await?;

        if output.trim().is_empty() {
            eprintln!("  (no output from backend)");
            return Ok(pages_written);
        }

        let db = Database::open(&get_default_db_path())?;
        let sid = session.unwrap();
        let full_id = resolve_session_id(&db, sid)?;
        let session_ids = vec![full_id.clone()];

        // 페이지 경로: 프로젝트 정보로 결정
        let page_path = if let Ok((meta, _)) = db.get_session_with_turns(&full_id) {
            if let Some(proj) = &meta.project {
                let safe = safe_project_name(proj);
                if !safe.is_empty() {
                    format!("projects/{}.md", safe)
                } else {
                    format!("sessions/{}.md", &full_id[..full_id.len().min(8)])
                }
            } else {
                format!("sessions/{}.md", &full_id[..full_id.len().min(8)])
            }
        } else {
            format!("sessions/{}.md", &full_id[..full_id.len().min(8)])
        };

        let vault_paths = collect_vault_paths(&db, &session_ids);

        let validated = secall_core::wiki::lint::validate_frontmatter(&output, &session_ids);
        let merged = secall_core::wiki::lint::merge_with_existing(
            &wiki_dir,
            &page_path,
            &validated,
            &session_ids,
        )?;
        let wiki_pages = collect_wiki_pages(&wiki_dir);
        let linked = secall_core::wiki::lint::insert_obsidian_links(
            &merged,
            &session_ids,
            &vault_paths,
            &wiki_pages,
        );

        let full_path = wiki_dir.join(&page_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, &linked)?;
        // P36 rework — 새 페이지 작성 성공 시 카운트 +1 (review-regen 은 동일 파일 덮어쓰기).
        pages_written += 1;
        eprintln!("  Written: {}", full_path.display());

        match secall_core::wiki::lint::run_markdownlint(&full_path) {
            Ok(Some(msg)) => eprintln!("  Lint: {}", msg),
            Ok(None) => {}
            Err(e) => eprintln!("  Lint error (skipped): {}", e),
        }

        eprintln!("  ✓ Wiki update complete.");

        if review {
            // markdownlint가 파일을 수정했을 수 있으므로 최종 저장본을 다시 읽음
            let final_content =
                std::fs::read_to_string(&full_path).unwrap_or_else(|_| linked.clone());
            let source_summary = build_review_source(&db, &session_ids);
            let resolved_model = resolve_review_model(review_model, &config);
            let needs_regen = run_review(&resolved_model, &final_content, &source_summary).await;

            // error급 이슈 → 1회 재생성 후 재검수 (무한 루프 방지: 최대 1회)
            if needs_regen {
                // P36 — cancel check before regeneration LLM call
                if let Some(s) = sink {
                    if s.is_cancelled() {
                        s.message("취소 요청 — 재생성 직전 취소 (haiku incremental)")
                            .await;
                        return Ok(pages_written);
                    }
                }
                eprintln!("    Regenerating due to review errors...");
                match backend_box.generate(&prompt).await {
                    Ok(regen_output) if !regen_output.trim().is_empty() => {
                        let validated2 = secall_core::wiki::lint::validate_frontmatter(
                            &regen_output,
                            &session_ids,
                        );
                        let merged2 = secall_core::wiki::lint::merge_with_existing(
                            &wiki_dir,
                            &page_path,
                            &validated2,
                            &session_ids,
                        )
                        .unwrap_or(validated2);
                        let wiki_pages2 = collect_wiki_pages(&wiki_dir);
                        let linked2 = secall_core::wiki::lint::insert_obsidian_links(
                            &merged2,
                            &session_ids,
                            &vault_paths,
                            &wiki_pages2,
                        );
                        if let Err(e) = std::fs::write(&full_path, &linked2) {
                            eprintln!("    Write failed, skipping re-review: {e}");
                        } else {
                            // 재검수 (반환값 무시 — 재시도는 1회만)
                            run_review(&resolved_model, &linked2, &source_summary).await;
                        }
                    }
                    _ => eprintln!("    Regeneration skipped (empty output)"),
                }
            }
        }
    } else {
        // ── 비-haiku 백엔드: 기존 동작 (출력만) ──
        // P36 — cancel check just before LLM call
        if let Some(s) = sink {
            if s.is_cancelled() {
                s.message(&format!(
                    "취소 요청 — LLM 호출 직전 취소 ({})",
                    backend_name
                ))
                .await;
                return Ok(pages_written);
            }
        }
        eprintln!("  Launching {}...", backend_box.name());
        let output = backend_box.generate(&prompt).await?;

        if output.trim().is_empty() {
            eprintln!("  (no output from backend)");
            return Ok(pages_written);
        }

        println!("{}", output);
        eprintln!("  ✓ Wiki update complete.");
    }

    Ok(pages_written)
}

fn resolve_backend_model(config: &Config, backend_name: &str, cli_model: Option<&str>) -> String {
    if let Some(model) = cli_model {
        return model.to_string();
    }

    if let Some(model) = config.wiki_backend_config(backend_name).model {
        return model;
    }

    match backend_name {
        "claude" => "sonnet".to_string(),
        "codex" => "gpt-5.4".to_string(),
        _ => String::new(),
    }
}

pub fn run_status() -> Result<()> {
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");

    if !wiki_dir.exists() {
        println!("Wiki not initialized. Run `secall init`.");
        return Ok(());
    }

    let mut page_count = 0;
    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
            page_count += 1;
        }
    }

    println!("Wiki: {}", wiki_dir.display());
    println!("Pages: {page_count}");
    Ok(())
}

// ─── Haiku 프롬프트 구성 ──────────────────────────────────────────────────

/// Haiku 백엔드용 프롬프트 — 세션 데이터를 DB에서 직접 추출하여 주입
fn build_haiku_prompt(
    config: &Config,
    wiki_dir: &std::path::Path,
    session: Option<&str>,
    since: Option<&str>,
) -> Result<String> {
    let db = Database::open(&get_default_db_path())?;

    if let Some(sid) = session {
        build_haiku_incremental_prompt(&db, sid, wiki_dir)
    } else {
        build_haiku_batch_prompt(&db, config, since)
    }
}

/// 인크리멘탈 모드: 단일 세션 전문을 프롬프트에 주입
fn build_haiku_incremental_prompt(
    db: &Database,
    session_id: &str,
    wiki_dir: &std::path::Path,
) -> Result<String> {
    // 접두사 매칭 허용 (8자리 이상)
    let full_id = resolve_session_id(db, session_id)?;
    let (meta, turns) = db.get_session_with_turns(&full_id)?;

    let mut prompt = format!(
        "## 세션 정보\n\
         - ID: {}\n\
         - 에이전트: {}\n\
         - 프로젝트: {}\n\
         - 날짜: {}\n\
         - 턴 수: {}\n\
         - 도구: {}\n\
         - 요약: {}\n\n\
         ## 대화 내용\n\n",
        meta.id,
        meta.agent,
        meta.project.as_deref().unwrap_or("(없음)"),
        &meta.start_time[..10.min(meta.start_time.len())],
        meta.turn_count,
        meta.tools_used.as_deref().unwrap_or("(없음)"),
        meta.summary.as_deref().unwrap_or("(없음)"),
    );

    for turn in &turns {
        let role_label = match turn.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            _ => "System",
        };
        prompt.push_str(&format!(
            "### Turn {} — {}\n\n",
            turn.turn_index, role_label
        ));
        // 턴 내용 제한: 각 턴 최대 4KB
        let content = if turn.content.len() > 4000 {
            format!("{}...(truncated)", &turn.content[..4000])
        } else {
            turn.content.clone()
        };
        prompt.push_str(&content);
        prompt.push_str("\n\n");
    }

    // 기존 위키 페이지 목록 주입 (병합 힌트, 최대 50개)
    let existing_pages: Vec<String> = walkdir::WalkDir::new(wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .filter_map(|e| {
            e.path()
                .strip_prefix(wiki_dir)
                .ok()
                .map(|rel| rel.to_string_lossy().to_string())
        })
        .collect();

    if !existing_pages.is_empty() {
        prompt.push_str("## 기존 위키 페이지 목록 (병합 참고용)\n\n");
        for page in existing_pages.iter().take(50) {
            prompt.push_str(&format!("- {}\n", page));
        }
        prompt.push_str(
            "\n위 페이지가 이 세션과 관련이 있으면 새 페이지를 만들지 말고 \
             기존 페이지에 통합하도록 판단하세요.\n\n",
        );
    }

    prompt.push_str("위 세션을 바탕으로 위키 페이지를 작성하세요.");
    Ok(prompt)
}

/// 배치 모드: since 기준 세션들을 프로젝트별로 그룹핑하여 프롬프트 구성
fn build_haiku_batch_prompt(
    db: &Database,
    _config: &Config,
    since: Option<&str>,
) -> Result<String> {
    let since_date = since.unwrap_or("2000-01-01");
    let sessions = db.get_sessions_since(since_date)?;

    if sessions.is_empty() {
        anyhow::bail!("No sessions found since {}", since_date);
    }

    // 프로젝트별 그룹핑
    let mut by_project: std::collections::BTreeMap<
        String,
        Vec<&secall_core::store::db::SessionMeta>,
    > = std::collections::BTreeMap::new();
    for s in &sessions {
        let proj = s.project.as_deref().unwrap_or("(기타)").to_string();
        by_project.entry(proj).or_default().push(s);
    }

    let mut prompt = format!(
        "## 위키 생성 대상: {} 이후 세션 {}개\n\n",
        since_date,
        sessions.len()
    );

    for (proj, proj_sessions) in &by_project {
        prompt.push_str(&format!("### 프로젝트: {}\n\n", proj));
        for s in proj_sessions {
            let date = &s.start_time[..10.min(s.start_time.len())];
            let summary = s.summary.as_deref().unwrap_or("(요약 없음)");
            let summary_short: String = summary
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect();
            prompt.push_str(&format!(
                "#### {} ({}, {}턴, {})\n{}\n\n",
                &s.id[..8.min(s.id.len())],
                date,
                s.turn_count,
                s.agent,
                summary_short,
            ));

            // 턴 내용 주입 (최대 3KB)
            if let Ok((_, turns)) = db.get_session_with_turns(&s.id) {
                let mut turn_text = String::new();
                for turn in &turns {
                    let role_label = match turn.role.as_str() {
                        "user" => "User",
                        "assistant" => "Assistant",
                        _ => "System",
                    };
                    let snippet = if turn.content.len() > 1000 {
                        format!("{}...", &turn.content[..1000])
                    } else {
                        turn.content.clone()
                    };
                    turn_text.push_str(&format!("**{}**: {}\n", role_label, snippet));
                    if turn_text.len() > 3000 {
                        turn_text.push_str("...(truncated)\n");
                        break;
                    }
                }
                if !turn_text.is_empty() {
                    prompt.push_str("<details>\n<summary>대화 내용</summary>\n\n");
                    prompt.push_str(&turn_text);
                    prompt.push_str("\n</details>\n\n");
                }
            }
        }
        prompt.push('\n');
    }

    prompt.push_str(
        "위 세션 목록을 바탕으로 프로젝트별 위키 페이지를 작성하세요.\n\
         각 프로젝트마다 별도의 마크다운 파일로 출력하세요.\n\
         각 파일은 `---` 구분선으로 구분하세요.",
    );
    Ok(prompt)
}

/// 세션 ID 접두사 → 전체 ID 해석
fn resolve_session_id(db: &Database, prefix: &str) -> Result<String> {
    if prefix.len() >= 36 {
        return Ok(prefix.to_string());
    }
    let pattern = format!("{}%", prefix);
    let results: Vec<String> = db
        .conn()
        .prepare("SELECT id FROM sessions WHERE id LIKE ?1")?
        .query_map([pattern], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    match results.len() {
        0 => anyhow::bail!("No session found matching '{}'", prefix),
        1 => Ok(results.into_iter().next().unwrap()),
        n => anyhow::bail!(
            "Ambiguous session prefix '{}': {} matches. Use more characters.",
            prefix,
            n
        ),
    }
}

/// 세션 ID 목록 → vault 상대경로 매핑 수집 (Obsidian 링크용)
fn collect_vault_paths(
    db: &Database,
    session_ids: &[String],
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for sid in session_ids {
        if let Ok(Some(vp)) = db.get_session_vault_path(sid) {
            map.insert(sid.clone(), vp);
        }
    }
    map
}

// ─── 기존 백엔드용 프롬프트 (claude, ollama, lmstudio) ───────────────────

fn load_batch_prompt(since: Option<&str>) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-update.md");
    let mut prompt = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../../docs/prompts/wiki-update.md").to_string()
    };

    if let Some(since) = since {
        prompt.push_str(&format!(
            "\n\n## 추가 조건\n- `--since {since}` 이후 세션만 검색하세요.\n"
        ));
    }

    Ok(prompt)
}

fn load_incremental_prompt(session_id: &str) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-incremental.md");
    let template = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../../docs/prompts/wiki-incremental.md").to_string()
    };

    Ok(template
        .replace("{SECALL_SESSION_ID}", session_id)
        .replace(
            "{SECALL_AGENT}",
            &std::env::var("SECALL_AGENT").unwrap_or_default(),
        )
        .replace(
            "{SECALL_PROJECT}",
            &std::env::var("SECALL_PROJECT").unwrap_or_default(),
        )
        .replace(
            "{SECALL_DATE}",
            &std::env::var("SECALL_DATE").unwrap_or_default(),
        ))
}

fn load_haiku_system_prompt() -> String {
    let custom_path = prompt_dir().join("wiki-haiku-system.md");
    if custom_path.exists() {
        std::fs::read_to_string(&custom_path).unwrap_or_default()
    } else {
        include_str!("../../../../docs/prompts/wiki-haiku-system.md").to_string()
    }
}

/// 프로젝트명 → 파일명 안전 문자열
fn safe_project_name(name: &str) -> String {
    name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "-")
        .trim_matches('-')
        .to_string()
}

/// review_model 우선순위: CLI > config.wiki.review_model > "sonnet"
fn resolve_review_model(cli: Option<&str>, config: &Config) -> String {
    cli.map(|s| s.to_string())
        .or_else(|| config.wiki.review_model.clone())
        .unwrap_or_else(|| "sonnet".to_string())
}

/// 단일 프로젝트용 Haiku 프롬프트 (배치 모드에서 프로젝트별 호출용)
fn build_haiku_single_project_prompt(
    db: &Database,
    project_name: &str,
    sessions: &[&secall_core::store::db::SessionMeta],
) -> Result<String> {
    let mut prompt = format!(
        "## 프로젝트: {}\n## 세션 {}개\n\n",
        project_name,
        sessions.len()
    );

    for s in sessions {
        let date = &s.start_time[..10.min(s.start_time.len())];
        let summary = s.summary.as_deref().unwrap_or("(요약 없음)");
        let summary_short: String = summary
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(200)
            .collect();
        prompt.push_str(&format!(
            "### {} ({}, {}턴, {})\n{}\n\n",
            &s.id[..8.min(s.id.len())],
            date,
            s.turn_count,
            s.agent,
            summary_short,
        ));

        // 턴 내용 주입 (최대 3KB)
        if let Ok((_, turns)) = db.get_session_with_turns(&s.id) {
            let mut turn_text = String::new();
            for turn in &turns {
                let role_label = match turn.role.as_str() {
                    "user" => "User",
                    "assistant" => "Assistant",
                    _ => "System",
                };
                let snippet = if turn.content.len() > 1000 {
                    format!("{}...", &turn.content[..1000])
                } else {
                    turn.content.clone()
                };
                turn_text.push_str(&format!("**{}**: {}\n", role_label, snippet));
                if turn_text.len() > 3000 {
                    turn_text.push_str("...(truncated)\n");
                    break;
                }
            }
            if !turn_text.is_empty() {
                prompt.push_str(&turn_text);
                prompt.push('\n');
            }
        }
    }

    prompt.push_str("위 세션들을 바탕으로 이 프로젝트의 위키 페이지를 작성하세요.");
    Ok(prompt)
}

/// 검수용 원본 세션 데이터 수집 (사실 정확성 대조용)
fn build_review_source(db: &Database, session_ids: &[String]) -> String {
    let mut summary = String::new();
    for sid in session_ids {
        if let Ok((meta, turns)) = db.get_session_with_turns(sid) {
            summary.push_str(&format!(
                "### Session {} ({})\n- Agent: {}\n- Summary: {}\n",
                &sid[..sid.len().min(8)],
                &meta.start_time[..10.min(meta.start_time.len())],
                meta.agent,
                meta.summary.as_deref().unwrap_or("N/A"),
            ));
            let mut turn_len = 0;
            for turn in turns.iter().take(5) {
                let snippet = if turn.content.len() > 500 {
                    format!("{}...", &turn.content[..500])
                } else {
                    turn.content.clone()
                };
                summary.push_str(&format!(
                    "- Turn {} ({}): {}\n",
                    turn.turn_index, turn.role, snippet
                ));
                turn_len += snippet.len();
                if turn_len > 2000 {
                    break;
                }
            }
            summary.push('\n');
        }
    }
    if summary.is_empty() {
        "No source session data available".to_string()
    } else {
        summary
    }
}

/// --review 검수 실행. error급 이슈가 있으면 true(재생성 필요), 없거나 API 실패 시 false 반환
async fn run_review(model: &str, page_content: &str, source_summary: &str) -> bool {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("  ⚠ Review skipped: ANTHROPIC_API_KEY not set");
            return false;
        }
    };

    eprintln!("  Reviewing with {}...", model);
    match secall_core::wiki::review::review_page(&api_key, model, page_content, source_summary)
        .await
    {
        Ok(result) => {
            if result.approved {
                eprintln!("  ✓ Review: approved");
                false
            } else {
                let error_count = result
                    .issues
                    .iter()
                    .filter(|i| i.severity == "error")
                    .count();
                eprintln!(
                    "  ⚠ Review: {} issue(s) found ({} error)",
                    result.issues.len(),
                    error_count
                );
                for issue in &result.issues {
                    eprintln!("    [{}] {}", issue.severity, issue.description);
                    if let Some(ref sug) = issue.suggestion {
                        eprintln!("      → {}", sug);
                    }
                }
                error_count > 0
            }
        }
        Err(e) => {
            eprintln!("  ⚠ Review failed (skipped): {}", e);
            false
        }
    }
}

/// wiki/ 디렉토리를 스캔하여 페이지 경로 목록 반환 (확장자 제거, Obsidian 링크용)
fn collect_wiki_pages(wiki_dir: &std::path::Path) -> Vec<String> {
    walkdir::WalkDir::new(wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .filter_map(|e| {
            e.path()
                .strip_prefix(wiki_dir)
                .ok()
                .map(|rel| rel.with_extension("").to_string_lossy().to_string())
        })
        .collect()
}

fn prompt_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_PROMPTS_DIR") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("prompts")
}
