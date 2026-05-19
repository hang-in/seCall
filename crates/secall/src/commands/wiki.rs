use std::path::PathBuf;

use anyhow::Result;
use secall_core::{
    jobs::ProgressSink,
    llm::defaults::{
        warn_using_default, WIKI_CLAUDE_DEFAULT, WIKI_CODEX_DEFAULT, WIKI_REVIEW_DEFAULT,
    },
    search::OllamaEmbedder,
    store::{get_default_db_path, Database},
    vault::{git::VaultGit, Config},
    wiki::WikiIndexer,
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
    pub review_backend: Option<String>,
    pub review_model: Option<String>,
    #[serde(default)]
    pub no_pull: bool,
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
    let target_label = build_target_label(args.session.as_deref(), args.since.as_deref());

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
        args.review_backend.as_deref(),
        args.review_model.as_deref(),
        args.no_pull,
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

#[allow(clippy::too_many_arguments)]
pub async fn run_update(
    model: Option<&str>,
    backend: Option<&str>,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
    review: bool,
    review_backend: Option<&str>,
    review_model: Option<&str>,
    no_pull: bool,
) -> Result<()> {
    // P36 rework — run_update_with_sink 가 page count 반환하지만 CLI 경로에서는 무시.
    run_update_with_sink(
        model,
        backend,
        since,
        session,
        dry_run,
        review,
        review_backend,
        review_model,
        no_pull,
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
    review_backend: Option<&str>,
    review_model: Option<&str>,
    no_pull: bool,
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

    preflight_vault_git(&config, dry_run, no_pull).await?;

    // 4. 백엔드 선택: --backend 플래그 → config wiki.default_backend → "claude"
    let backend_name = backend
        .map(|s| s.to_string())
        .unwrap_or_else(|| config.wiki.default_backend.clone());
    let resolved_model = resolve_backend_model(&config, &backend_name, model);

    // P86 (issue #88): ollama / lmstudio 백엔드는 wiki update 의 prompt 가
    // 가정하는 MCP 도구 호출 (`secall recall/get/status` + `wiki/` 파일 쓰기) 을
    // 수행할 능력이 없다. batch / incremental / --since 모든 모드에서 모델이
    // "임무 이해" 응답만 출력하고 종료 → 사용자가 silent wait (timeout 까지)
    // 후 빈손으로 깨닫는 사고. 명시적 fail-fast + 가이드.
    if matches!(backend_name.as_str(), "ollama" | "lmstudio") {
        anyhow::bail!(
            "wiki backend `{backend_name}` 는 wiki update 와 호환되지 않습니다.\n\
             도구 호출 (`secall recall`/`get`/`status` + `wiki/` 파일 쓰기) 능력이 없어\n\
             prompt 가 가정하는 작업을 수행할 수 없습니다.\n\
             \n\
             해결책 (편한 것 사용):\n\
               • `--backend claude`  (Claude Code CLI + MCP 통합)\n\
               • `--backend codex`   (Codex CLI)\n\
               • `--backend haiku`   (Anthropic API, 세션 데이터를 prompt 에 inline,\n\
                                      `ANTHROPIC_API_KEY` 필요)\n\
             \n\
             참고: issue #88, docs/plans/p86-ollama-batch-fail-fast.md"
        );
    }

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

    // P53: --since 옵션을 target 표시에도 반영 (이전: session 만 보고, since 는
    // 무조건 "all sessions" 로 표시되어 사용자 혼란).
    let target = build_target_label(session, since);
    eprintln!("Wiki update: {} (backend: {})", target, backend_name);

    // 5. WikiBackend 인스턴스 생성
    let backend_box = build_wiki_backend(&config, &backend_name, &resolved_model)?;

    // 6. 생성 + 후처리
    if backend_name == "haiku" && session.is_none() {
        pages_written = process_haiku_batch(
            &config,
            &wiki_dir,
            since,
            backend_box.as_ref(),
            review,
            review_backend,
            review_model,
            sink,
        )
        .await?;
    } else if backend_name == "haiku" {
        pages_written = process_haiku_incremental(
            &config,
            &wiki_dir,
            session.unwrap(),
            &prompt,
            backend_box.as_ref(),
            review,
            review_backend,
            review_model,
            sink,
        )
        .await?;
    } else {
        process_generic_backend(&prompt, backend_box.as_ref(), &backend_name, sink).await?;
    }

    Ok(pages_written)
}

/// `run_update_with_sink` 분해 (helper 1/4): vault git preflight.
///
/// - conflicted state 검사 → bail
/// - dry_run/no_pull 가 아니면 auto_commit + pull + wiki 충돌 자동 해결
async fn preflight_vault_git(config: &Config, dry_run: bool, no_pull: bool) -> Result<()> {
    let vault_git = VaultGit::new(&config.vault.path, &config.vault.branch);
    if vault_git.is_git_repo() {
        if let Some(msg) = vault_git.check_conflicted_state() {
            anyhow::bail!("wiki update aborted - vault git conflict detected.\n\n{msg}");
        }

        if !dry_run && !no_pull {
            match vault_git.auto_commit() {
                Ok(true) => eprintln!("Auto-committed unstaged vault changes before pull."),
                Ok(false) => {}
                Err(e) => eprintln!("Warning: auto-commit failed: {e}"),
            }
            match vault_git.pull() {
                Ok(result) if result.already_up_to_date => {}
                Ok(result) => eprintln!("Pulled vault: {} new session file(s).", result.new_files),
                Err(e) => eprintln!("Warning: vault pull failed: {e}"),
            }

            let unmerged = vault_git.unmerged_files().unwrap_or_default();
            if !unmerged.is_empty() {
                let (wiki_conflicts, non_wiki_conflicts): (Vec<_>, Vec<_>) = unmerged
                    .into_iter()
                    .partition(|path| path.starts_with("wiki/") && path.ends_with(".md"));

                if !non_wiki_conflicts.is_empty() {
                    anyhow::bail!(
                        "wiki update aborted - non-wiki conflicts pending:\n{}\nResolve manually then re-run.",
                        non_wiki_conflicts.join("\n")
                    );
                }

                if !wiki_conflicts.is_empty() {
                    eprintln!(
                        "Auto-resolving {} wiki conflict(s) via sources union regeneration...",
                        wiki_conflicts.len()
                    );
                    let resolved =
                        auto_resolve_wiki_conflicts(config, &vault_git, &wiki_conflicts).await?;
                    eprintln!("Resolved {resolved} wiki conflict(s).");
                }
            }
        }
    }
    Ok(())
}

/// `run_update_with_sink` 분해 (helper 2/4): haiku 배치 모드 — 프로젝트별 개별 호출.
///
/// 세션을 프로젝트별로 묶어 각각 LLM 호출하고 page 작성 + 옵션 review.
/// 반환값은 새로 작성된 페이지 개수 (review-regen 은 카운트 X).
#[allow(clippy::too_many_arguments)]
async fn process_haiku_batch(
    config: &Config,
    wiki_dir: &std::path::Path,
    since: Option<&str>,
    backend: &dyn secall_core::wiki::WikiBackend,
    review: bool,
    review_backend: Option<&str>,
    review_model: Option<&str>,
    sink: Option<&dyn ProgressSink>,
) -> Result<usize> {
    let mut pages_written: usize = 0;
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

    let resolved_review_backend = resolve_review_backend(review_backend, config);
    let resolved_model = resolve_review_model(review_model, config, &resolved_review_backend);
    let reviewer = build_reviewer(config, &resolved_review_backend, &resolved_model)?;

    // Gemini PR #59: 매 페이지 작성마다 wiki/ 전체를 재귀 스캔하면 O(N²) 부하.
    // 루프 시작 시 한 번 수집한 뒤, 새 페이지를 작성할 때마다 list 에 추가한다.
    let mut wiki_pages = collect_wiki_pages(wiki_dir);

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
        let output = backend.generate(&proj_prompt).await?;

        if output.trim().is_empty() {
            eprintln!("    (no output, skipping)");
            continue;
        }

        let page_path = format!("projects/{}.md", safe_project_name(proj_name));

        let (full_path, linked) = write_wiki_page(
            wiki_dir,
            &page_path,
            &output,
            &session_ids,
            &vault_paths,
            &wiki_pages,
            "    ",
        )?;
        // 새로 만든 페이지를 link 풀에 추가 — 다음 iteration 의 페이지가
        // 이 페이지를 wikilink 로 참조할 수 있도록.
        let new_link = page_path.trim_end_matches(".md").to_string();
        if !wiki_pages.contains(&new_link) {
            wiki_pages.push(new_link);
        }
        // P36 rework — 새 페이지 작성 성공 시 카운트 +1.
        // (review-regen 은 같은 파일 덮어쓰기라 카운트 증가 안 함)
        pages_written += 1;
        eprintln!("    Written: {}", full_path.display());

        if review {
            maybe_review_with_regen(
                backend,
                reviewer.as_ref(),
                &db,
                wiki_dir,
                &page_path,
                &proj_prompt,
                &session_ids,
                &vault_paths,
                &wiki_pages,
                &full_path,
                &linked,
                sink,
                &format!("{} 프로젝트", proj_name),
                "    ",
            )
            .await?;
        }
    }
    eprintln!(
        "  ✓ Wiki batch update complete ({} projects).",
        by_project.len()
    );
    Ok(pages_written)
}

/// `run_update_with_sink` 분해 (helper 3/4): haiku 인크리멘탈 모드 — 단일 세션.
///
/// 지정한 세션 1개를 LLM 으로 생성, 프로젝트 정보 기반 page_path 결정,
/// 작성 후 옵션 review. 반환값은 새 페이지 카운트 (0/1).
#[allow(clippy::too_many_arguments)]
async fn process_haiku_incremental(
    config: &Config,
    wiki_dir: &std::path::Path,
    session_id: &str,
    prompt: &str,
    backend: &dyn secall_core::wiki::WikiBackend,
    review: bool,
    review_backend: Option<&str>,
    review_model: Option<&str>,
    sink: Option<&dyn ProgressSink>,
) -> Result<usize> {
    let mut pages_written: usize = 0;

    // P36 — cancel check just before LLM call
    if let Some(s) = sink {
        if s.is_cancelled() {
            s.message("취소 요청 — LLM 호출 직전 취소 (haiku incremental)")
                .await;
            return Ok(pages_written);
        }
    }
    eprintln!("  Launching {}...", backend.name());
    let output = backend.generate(prompt).await?;

    if output.trim().is_empty() {
        eprintln!("  (no output from backend)");
        return Ok(pages_written);
    }

    let db = Database::open(&get_default_db_path())?;
    let full_id = resolve_session_id(&db, session_id)?;
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
    // incremental 모드는 한 페이지만 작성 → caller 가 한 번만 수집.
    let wiki_pages = collect_wiki_pages(wiki_dir);

    let (full_path, linked) = write_wiki_page(
        wiki_dir,
        &page_path,
        &output,
        &session_ids,
        &vault_paths,
        &wiki_pages,
        "  ",
    )?;
    // P36 rework — 새 페이지 작성 성공 시 카운트 +1 (review-regen 은 동일 파일 덮어쓰기).
    pages_written += 1;
    eprintln!("  Written: {}", full_path.display());

    eprintln!("  ✓ Wiki update complete.");

    if review {
        let resolved_review_backend = resolve_review_backend(review_backend, config);
        let resolved_model = resolve_review_model(review_model, config, &resolved_review_backend);
        let reviewer = build_reviewer(config, &resolved_review_backend, &resolved_model)?;
        maybe_review_with_regen(
            backend,
            reviewer.as_ref(),
            &db,
            wiki_dir,
            &page_path,
            prompt,
            &session_ids,
            &vault_paths,
            &wiki_pages,
            &full_path,
            &linked,
            sink,
            "haiku incremental",
            "    ",
        )
        .await?;
    }

    Ok(pages_written)
}

/// `run_update_with_sink` 분해 (helper 4/4): 비-haiku 백엔드 (claude/codex/ollama/lmstudio).
///
/// LLM 호출 직전 cancel 체크, 호출 후 출력을 stdout 으로 그대로 흘려보낸다.
/// 페이지 작성/카운트는 하지 않는다.
async fn process_generic_backend(
    prompt: &str,
    backend: &dyn secall_core::wiki::WikiBackend,
    backend_name: &str,
    sink: Option<&dyn ProgressSink>,
) -> Result<()> {
    // P36 — cancel check just before LLM call
    if let Some(s) = sink {
        if s.is_cancelled() {
            s.message(&format!(
                "취소 요청 — LLM 호출 직전 취소 ({})",
                backend_name
            ))
            .await;
            return Ok(());
        }
    }
    eprintln!("  Launching {}...", backend.name());
    let output = backend.generate(prompt).await?;

    if output.trim().is_empty() {
        eprintln!("  (no output from backend)");
        return Ok(());
    }

    println!("{}", output);
    eprintln!("  ✓ Wiki update complete.");
    Ok(())
}

async fn auto_resolve_wiki_conflicts(
    config: &Config,
    vault_git: &VaultGit<'_>,
    paths: &[String],
) -> Result<usize> {
    let db = Database::open(&get_default_db_path())?;
    let backend_name = config.wiki.default_backend.clone();
    let resolved_model = resolve_backend_model(config, &backend_name, None);
    let backend = build_wiki_backend(config, &backend_name, &resolved_model)?;
    let wiki_dir = config.vault.path.join("wiki");

    let mut resolved = 0usize;
    for path in paths {
        let sources = vault_git.extract_sources_from_conflicted(path)?;
        if sources.is_empty() {
            anyhow::bail!("auto-resolve failed for {path}: no frontmatter sources found");
        }

        let prompt = build_conflict_resolution_prompt(&db, &wiki_dir, path, &sources)?;
        let output = backend.generate(&prompt).await?;
        if output.trim().is_empty() {
            anyhow::bail!("auto-resolve failed for {path}: backend returned empty output");
        }

        let validated = secall_core::wiki::lint::validate_frontmatter(&output, &sources);
        let wiki_pages = collect_wiki_pages(&wiki_dir);
        let vault_paths = collect_vault_paths(&db, &sources);
        let linked = secall_core::wiki::lint::insert_obsidian_links(
            &validated,
            &sources,
            &vault_paths,
            &wiki_pages,
        );

        let full_path = config.vault.path.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, linked)?;
        vault_git.stage_resolved(path)?;
        resolved += 1;
    }

    vault_git.finish_conflict_resolution("auto-resolve wiki conflicts")?;
    Ok(resolved)
}

fn build_wiki_backend(
    config: &Config,
    backend_name: &str,
    resolved_model: &str,
) -> Result<Box<dyn secall_core::wiki::WikiBackend>> {
    match backend_name {
        "haiku" => {
            let cfg = config.wiki_backend_config("haiku");
            let system_prompt = load_haiku_system_prompt();
            Ok(Box::new(secall_core::wiki::HaikuBackend::from_env(
                cfg.model,
                cfg.max_tokens,
                system_prompt,
            )?))
        }
        "ollama" => {
            let cfg = config.wiki_backend_config("ollama");
            Ok(Box::new(secall_core::wiki::OllamaBackend {
                api_url: cfg
                    .api_url
                    .unwrap_or_else(|| "http://localhost:11434".to_string()),
                model: cfg.model.unwrap_or_else(|| "llama3".to_string()),
                max_tokens: cfg.max_tokens,
                api_key: None,
            }))
        }
        // P55 Gemini #64: build_reviewer 와 일관되게 ollama_cloud 도 지원.
        // P56: wiki backend config 의 cloud_api_key / cloud_host 우선 (graph/log
        // 와 분리해 wiki 전용 키 / 엔드포인트 가능, Gemini PR #64 MEDIUM 반영).
        "ollama_cloud" => {
            let cfg = config.wiki_backend_config("ollama_cloud");
            let api_key = cfg
                .cloud_api_key
                .clone()
                .or_else(|| std::env::var("OLLAMA_CLOUD_API_KEY").ok())
                .or_else(|| config.graph.cloud_api_key.clone())
                .or_else(|| config.log.cloud_api_key.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ollama cloud api key not set \
                         (set [wiki.backends.ollama_cloud].cloud_api_key, \
                         OLLAMA_CLOUD_API_KEY env, or [graph].cloud_api_key in config.toml)"
                    )
                })?;
            Ok(Box::new(secall_core::wiki::OllamaBackend {
                api_url: cfg
                    .api_url
                    .or(cfg.cloud_host)
                    .or_else(|| config.graph.cloud_host.clone())
                    .unwrap_or_else(|| "https://ollama.com".to_string()),
                model: cfg.model.unwrap_or_else(|| {
                    secall_core::llm::defaults::WIKI_REVIEW_OLLAMA_CLOUD_DEFAULT.to_string()
                }),
                max_tokens: cfg.max_tokens,
                api_key: Some(api_key),
            }))
        }
        "lmstudio" => {
            let cfg = config.wiki_backend_config("lmstudio");
            Ok(Box::new(secall_core::wiki::LmStudioBackend {
                api_url: cfg
                    .api_url
                    .unwrap_or_else(|| "http://localhost:1234".to_string()),
                model: cfg.model.unwrap_or_else(|| "local-model".to_string()),
                max_tokens: cfg.max_tokens,
            }))
        }
        "codex" => Ok(Box::new(secall_core::wiki::CodexBackend {
            model: resolved_model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        "claude" => Ok(Box::new(secall_core::wiki::ClaudeBackend {
            model: resolved_model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        _ => anyhow::bail!(
            "Unknown backend '{}'. Supported: claude, codex, haiku, ollama, ollama_cloud, lmstudio",
            backend_name
        ),
    }
}

fn build_conflict_resolution_prompt(
    db: &Database,
    wiki_dir: &std::path::Path,
    path: &str,
    session_ids: &[String],
) -> Result<String> {
    let page_hint = path.strip_prefix("wiki/").unwrap_or(path);
    let mut prompt = format!(
        "Regenerate the canonical wiki page for this conflicted path.\n\
         Target page: {page_hint}\n\
         Output only the final markdown page with YAML frontmatter.\n\
         The `sources` field must include every provided session ID exactly once.\n\
         Replace any prior body entirely and do not mention merge conflicts.\n\n"
    );

    let existing_pages: Vec<String> = walkdir::WalkDir::new(wiki_dir)
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
            entry
                .path()
                .strip_prefix(wiki_dir)
                .ok()
                .map(|rel| rel.to_string_lossy().to_string())
        })
        .collect();
    if !existing_pages.is_empty() {
        prompt.push_str("Existing wiki pages:\n");
        for page in existing_pages.iter().take(50) {
            prompt.push_str(&format!("- {page}\n"));
        }
        prompt.push('\n');
    }

    for session_id in session_ids {
        let (meta, turns) = db.get_session_with_turns(session_id)?;
        prompt.push_str(&format!(
            "## Session {}\n- Agent: {}\n- Project: {}\n- Date: {}\n- Summary: {}\n\n",
            meta.id,
            meta.agent,
            meta.project.as_deref().unwrap_or("(none)"),
            &meta.start_time[..10.min(meta.start_time.len())],
            meta.summary.as_deref().unwrap_or("(none)"),
        ));
        for turn in turns.iter().take(8) {
            let snippet = if turn.content.len() > 800 {
                format!("{}...", &turn.content[..800])
            } else {
                turn.content.clone()
            };
            prompt.push_str(&format!(
                "### Turn {} ({})\n{}\n\n",
                turn.turn_index, turn.role, snippet
            ));
        }
    }

    prompt.push_str("Write the resolved wiki page now.");
    Ok(prompt)
}

fn resolve_backend_model(config: &Config, backend_name: &str, cli_model: Option<&str>) -> String {
    if let Some(model) = cli_model {
        return model.to_string();
    }

    if let Some(model) = config.wiki_backend_config(backend_name).model {
        return model;
    }

    match backend_name {
        "claude" => {
            warn_using_default("wiki.backends.claude.model", WIKI_CLAUDE_DEFAULT);
            WIKI_CLAUDE_DEFAULT.to_string()
        }
        "codex" => {
            warn_using_default("wiki.backends.codex.model", WIKI_CODEX_DEFAULT);
            WIKI_CODEX_DEFAULT.to_string()
        }
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

pub async fn vectorize(force: bool, model: &str, ollama_url: &str) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;
    let embedder = OllamaEmbedder::new(Some(ollama_url), Some(model));
    let indexer = WikiIndexer {
        vault_path: &config.vault.path,
        db: &db,
        embedder: &embedder,
        model_id: model,
    };

    println!("Scanning wiki pages under: {}", config.vault.path.display());
    let result = if force {
        indexer.reindex_all().await?
    } else {
        indexer.index_all().await?
    };

    println!(
        "Wiki vectorize complete: scanned={} indexed={} skipped={} deleted={} failed={}",
        result.scanned,
        result.indexed,
        result.skipped,
        result.deleted,
        result.failed.len()
    );

    for (path, err) in &result.failed {
        eprintln!("  FAIL {path}: {err}");
    }

    if !result.failed.is_empty() {
        anyhow::bail!("{} pages failed to index", result.failed.len());
    }

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

/// P53 + Gemini: wiki update 의 target 표시 라벨. session > since > "all sessions"
/// 우선순위. `run_with_progress` (REST/job sink 측) 과 `run_update_with_sink`
/// (CLI stderr 측) 양쪽에서 사용 — 중복 제거 + format 통일 (모두 `session {}`).
fn build_target_label(session: Option<&str>, since: Option<&str>) -> String {
    if let Some(sid) = session {
        format!("session {}", &sid[..sid.len().min(8)])
    } else if let Some(s) = since {
        format!("sessions since {s}")
    } else {
        "all sessions".to_string()
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

/// review_model 우선순위: CLI > config.wiki.review_model > backend별 기본값
pub fn resolve_review_model(cli: Option<&str>, config: &Config, backend_name: &str) -> String {
    if let Some(model) = cli {
        return model.to_string();
    }
    if let Some(model) = config.wiki.review_model.clone() {
        return model;
    }
    if let Some(model) = config.wiki_backend_config(backend_name).model {
        return model;
    }

    match backend_name {
        "claude" | "anthropic" | "sonnet" | "opus" => {
            warn_using_default("wiki.review_model", WIKI_REVIEW_DEFAULT);
            WIKI_REVIEW_DEFAULT.to_string()
        }
        "codex" => {
            warn_using_default("wiki.backends.codex.model", WIKI_CODEX_DEFAULT);
            WIKI_CODEX_DEFAULT.to_string()
        }
        "haiku" => {
            const HAIKU_REVIEW_DEFAULT: &str = "claude-haiku-4-5-20251001";
            warn_using_default("wiki.review_model", HAIKU_REVIEW_DEFAULT);
            HAIKU_REVIEW_DEFAULT.to_string()
        }
        "ollama" | "lmstudio" => config
            .graph
            .ollama_model
            .clone()
            .unwrap_or_else(|| "gemma4:e4b".to_string()),
        // P55: ollama_cloud review default 는 kimi-k2.6:cloud (long context + JSON).
        "ollama_cloud" => config.graph.cloud_model.clone().unwrap_or_else(|| {
            warn_using_default(
                "wiki.review_model",
                secall_core::llm::defaults::WIKI_REVIEW_OLLAMA_CLOUD_DEFAULT,
            );
            secall_core::llm::defaults::WIKI_REVIEW_OLLAMA_CLOUD_DEFAULT.to_string()
        }),
        _ => {
            warn_using_default("wiki.review_model", WIKI_REVIEW_DEFAULT);
            WIKI_REVIEW_DEFAULT.to_string()
        }
    }
}

/// review_backend 우선순위: CLI > config.wiki.review_backend > default_backend > "haiku"
pub fn resolve_review_backend(cli: Option<&str>, config: &Config) -> String {
    if let Some(cli) = cli {
        return cli.to_string();
    }
    if let Some(configured) = config.wiki.review_backend.clone() {
        return configured;
    }
    if matches!(
        config.wiki.default_backend.as_str(),
        "claude" | "codex" | "haiku" | "ollama" | "ollama_cloud" | "lmstudio"
    ) {
        return config.wiki.default_backend.clone();
    }
    "haiku".to_string()
}

fn build_reviewer(
    config: &Config,
    backend_name: &str,
    model: &str,
) -> Result<Box<dyn secall_core::wiki::WikiReviewer>> {
    match backend_name {
        "anthropic" | "sonnet" | "opus" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            Ok(Box::new(secall_core::wiki::AnthropicReviewer {
                api_key,
                model: model.to_string(),
            }))
        }
        "claude" => Ok(Box::new(secall_core::wiki::ClaudeReviewer {
            model: model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        "codex" => Ok(Box::new(secall_core::wiki::CodexReviewer {
            model: model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        "haiku" => {
            if matches!(model, "sonnet" | "opus") {
                anyhow::bail!(
                    "review backend 'haiku' requires an Anthropic model id; leave review_model unset or set a value like claude-haiku-4-5-20251001"
                );
            }
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            Ok(Box::new(secall_core::wiki::HaikuReviewer {
                api_key,
                model: model.to_string(),
                max_tokens: 2048,
            }))
        }
        "ollama" => Ok(Box::new(secall_core::wiki::OllamaReviewer {
            api_url: config
                .wiki
                .backends
                .get("ollama")
                .and_then(|cfg| cfg.api_url.clone())
                .or_else(|| config.graph.ollama_url.clone())
                .unwrap_or_else(|| "http://localhost:11434".to_string()),
            model: model.to_string(),
            api_key: None,
        })),
        // P55: Ollama Cloud backend for wiki review.
        // P56: wiki backend config 의 cloud_api_key / cloud_host 우선 (Gemini PR #64 MEDIUM).
        "ollama_cloud" => {
            let cfg = config.wiki_backend_config("ollama_cloud");
            let api_key = cfg
                .cloud_api_key
                .clone()
                .or_else(|| std::env::var("OLLAMA_CLOUD_API_KEY").ok())
                .or_else(|| config.graph.cloud_api_key.clone())
                .or_else(|| config.log.cloud_api_key.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ollama cloud api key not set \
                         (set [wiki.backends.ollama_cloud].cloud_api_key, \
                         OLLAMA_CLOUD_API_KEY env, or [graph].cloud_api_key in config.toml)"
                    )
                })?;
            let api_url = cfg
                .cloud_host
                .or_else(|| config.graph.cloud_host.clone())
                .unwrap_or_else(|| "https://ollama.com".to_string());
            Ok(Box::new(secall_core::wiki::OllamaReviewer {
                api_url,
                model: model.to_string(),
                api_key: Some(api_key),
            }))
        }
        "lmstudio" => Ok(Box::new(secall_core::wiki::LmStudioReviewer {
            api_url: config
                .wiki
                .backends
                .get("lmstudio")
                .and_then(|cfg| cfg.api_url.clone())
                // Until a dedicated review URL is added, reuse the shared
                // local OpenAI-compatible URL fallback used by graph/log.
                .or_else(|| config.graph.ollama_url.clone())
                .unwrap_or_else(|| "http://localhost:1234".to_string()),
            model: model.to_string(),
        })),
        other => anyhow::bail!("unknown review backend: {other}"),
    }
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
async fn run_review(
    reviewer: &dyn secall_core::wiki::WikiReviewer,
    page_content: &str,
    source_summary: &str,
) -> bool {
    eprintln!("  Reviewing generated wiki page...");
    match reviewer.review(page_content, source_summary).await {
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

/// P50-C: wiki 페이지 markdown 을 validate → merge → obsidian links 삽입 →
/// disk write → markdownlint 까지 수행. `run_update_with_sink` 의 haiku
/// batch / haiku incremental 양쪽 흐름에서 동일하게 반복되던 ~30 lines 블록을
/// 한 곳으로 모았다.
///
/// 반환: 디스크에 쓰여진 `(절대 경로, 최종 markdown 본문)`. 호출자는 후속
/// review/regen 단계에서 본문을 다시 읽거나 그대로 사용한다.
#[allow(clippy::too_many_arguments)]
fn write_wiki_page(
    wiki_dir: &std::path::Path,
    page_path: &str,
    output: &str,
    session_ids: &[String],
    vault_paths: &std::collections::HashMap<String, String>,
    wiki_pages: &[String],
    log_indent: &str,
) -> Result<(PathBuf, String)> {
    let validated = secall_core::wiki::lint::validate_frontmatter(output, session_ids);
    let merged =
        secall_core::wiki::lint::merge_with_existing(wiki_dir, page_path, &validated, session_ids)?;
    let linked = secall_core::wiki::lint::insert_obsidian_links(
        &merged,
        session_ids,
        vault_paths,
        wiki_pages,
    );

    let full_path = wiki_dir.join(page_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full_path, &linked)?;

    match secall_core::wiki::lint::run_markdownlint(&full_path) {
        Ok(Some(msg)) => eprintln!("{log_indent}Lint: {}", msg),
        Ok(None) => {}
        Err(e) => eprintln!("{log_indent}Lint error (skipped): {}", e),
    }

    Ok((full_path, linked))
}

/// P50-C: review 가 error 급 이슈를 잡으면 1회만 재생성 + 재검수.
/// haiku batch / haiku incremental 양쪽 흐름에서 동일했던 ~50 lines 블록을 묶었다.
///
/// `initial_full_path` 가 markdownlint 단계에서 수정됐을 가능성을 고려해 디스크
/// 내용을 다시 읽어 review 에 전달한다 (실패 시 `initial_linked` fallback).
#[allow(clippy::too_many_arguments)]
async fn maybe_review_with_regen(
    backend: &dyn secall_core::wiki::WikiBackend,
    reviewer: &dyn secall_core::wiki::WikiReviewer,
    db: &Database,
    wiki_dir: &std::path::Path,
    page_path: &str,
    prompt: &str,
    session_ids: &[String],
    vault_paths: &std::collections::HashMap<String, String>,
    wiki_pages: &[String],
    initial_full_path: &std::path::Path,
    initial_linked: &str,
    sink: Option<&dyn ProgressSink>,
    cancel_label: &str,
    log_indent: &str,
) -> Result<()> {
    let final_content =
        std::fs::read_to_string(initial_full_path).unwrap_or_else(|_| initial_linked.to_string());
    let source_summary = build_review_source(db, session_ids);
    let needs_regen = run_review(reviewer, &final_content, &source_summary).await;

    if !needs_regen {
        return Ok(());
    }

    if let Some(s) = sink {
        if s.is_cancelled() {
            s.message(&format!("취소 요청 — 재생성 직전 취소 ({cancel_label})"))
                .await;
            return Ok(());
        }
    }
    eprintln!("{log_indent}Regenerating due to review errors...");
    match backend.generate(prompt).await {
        Ok(regen_output) if !regen_output.trim().is_empty() => {
            let (full_path2, linked2) = write_wiki_page(
                wiki_dir,
                page_path,
                &regen_output,
                session_ids,
                vault_paths,
                wiki_pages,
                log_indent,
            )?;
            // Gemini PR #59: 재검수도 첫 검수처럼 markdownlint 가 수정했을 가능성을
            // 고려해 디스크 최종본을 다시 읽어 review 에 전달한다.
            let final_content2 =
                std::fs::read_to_string(&full_path2).unwrap_or_else(|_| linked2.clone());
            // 재검수 (반환값 무시 — 재시도는 1회만)
            run_review(reviewer, &final_content2, &source_summary).await;
        }
        _ => eprintln!("{log_indent}Regeneration skipped (empty output)"),
    }
    Ok(())
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
