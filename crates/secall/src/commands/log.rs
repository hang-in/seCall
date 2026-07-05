use anyhow::Result;
use secall_core::{
    llm::defaults::{
        warn_using_default, GRAPH_LMSTUDIO_DEFAULT, LOG_CONTEXT_CHAR_LIMIT,
        LOG_OLLAMA_CLOUD_DEFAULT, LOG_OLLAMA_DEFAULT, WIKI_CLAUDE_DEFAULT, WIKI_CODEX_DEFAULT,
    },
    store::{get_default_db_path, Database},
    vault::Config,
    wiki::{
        ClaudeBackend, CodexBackend, HaikuBackend, LmStudioBackend, OllamaBackend, WikiBackend,
    },
};

pub async fn run(
    date: Option<String>,
    backend: Option<String>,
    model: Option<String>,
) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    // 날짜 결정 (기본: 오늘) — config timezone 기준
    let tz = config.timezone();
    let target_date = match date {
        Some(d) => d,
        None => chrono::Utc::now()
            .with_timezone(&tz)
            .format("%Y-%m-%d")
            .to_string(),
    };
    // config tz 의 UTC 오프셋(분) — get_sessions_for_date 가 UTC start_time 을 이 tz 로
    // 이동해 날짜를 비교(target_date 와 동일 tz 기준으로 일관).
    use chrono::Offset;
    let tz_offset_min = i64::from(
        chrono::Utc::now()
            .with_timezone(&tz)
            .offset()
            .fix()
            .local_minus_utc()
            / 60,
    );

    let sessions = db.get_sessions_for_date(&target_date, tz_offset_min)?;
    if sessions.is_empty() {
        eprintln!("No sessions found for {}", target_date);
        return Ok(());
    }

    // 자동화/노이즈 세션 필터링, 최소 2턴 이상
    let meaningful: Vec<_> = sessions
        .iter()
        .filter(|(_, _, _, turns, _, stype)| *turns >= 2 && stype != "automated")
        .collect();

    if meaningful.is_empty() {
        eprintln!(
            "No meaningful sessions for {} (all automated or < 2 turns)",
            target_date
        );
        return Ok(());
    }

    // 세션별 entry 를 시간 순(meaningful 의 원래 순서, get_sessions_for_date 가
    // ORDER BY start_time 보장)으로 빌드 — truncation 시 oldest 부터 잘라야
    // 하므로 (project, entry) 쌍을 시간순 유지하는 게 핵심.
    let session_ids: Vec<String> = meaningful
        .iter()
        .map(|(id, _, _, _, _, _)| id.clone())
        .collect();

    // (project, entry) pair, oldest-first 순서.
    let mut entries_chronological: Vec<(String, String)> = Vec::new();
    for (_id, project, summary, turns, tools, _) in &meaningful {
        let proj = project.as_deref().unwrap_or("(기타)").to_string();
        let summary_text = summary
            .as_deref()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(150)
            .collect::<String>();

        // 요약이 노이즈인 경우 스킵
        if summary_text.starts_with("Analyze the following")
            || summary_text.starts_with("<environment_context>")
            || summary_text.starts_with("<local-command-caveat>")
        {
            continue;
        }

        let tools_str = tools.as_deref().unwrap_or("[]");
        let entry = format!("- ({turns}턴, 도구:{tools_str}) {summary_text}");
        entries_chronological.push((proj, entry));
    }

    let by_project = group_entries_by_project(&entries_chronological);

    if by_project.is_empty() {
        eprintln!("No usable session summaries for {}", target_date);
        return Ok(());
    }

    // 시맨틱 토픽 조회 (graph에서)
    let topics = db.get_topics_for_sessions(&session_ids)?;
    let topic_labels: Vec<String> = topics
        .iter()
        .filter_map(|(_, t)| t.strip_prefix("topic:").map(|s| s.to_string()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let total = meaningful.len();
    let automated = sessions.len() - meaningful.len();

    let system_prompt = "당신은 개발자의 작업 일지를 작성하는 도우미입니다. \
        주어진 세션 요약을 바탕으로 그날 무엇을 했는지 자연스러운 한국어로 정리해주세요. \
        과장하지 말고 실제 작업 내용을 간결하게 서술하세요.";

    // diary 컨텍스트 가드 — kimi-k2.6 128k token (≈ 400k chars) 초과 방지.
    // 예산 초과 시 가장 오래된 (entries_chronological 의 앞쪽) session 부터
    // 1건씩 빼면서 user_prompt 재구성 — Plan(P46 Task 04) 요구사항(시간순) 충족.
    let system_chars = system_prompt.chars().count();
    let budget = LOG_CONTEXT_CHAR_LIMIT
        .saturating_sub(system_chars)
        .saturating_sub(1000); // 출력 여유분

    let mut chrono_view: &[(String, String)] = &entries_chronological;
    let mut dropped_oldest = 0usize;
    let mut user_prompt =
        build_diary_user_prompt(&target_date, chrono_view, &topic_labels, total, automated);

    while user_prompt.chars().count() > budget && !chrono_view.is_empty() {
        // 가장 오래된 entry 1건 제거 후 재빌드.
        chrono_view = &chrono_view[1..];
        dropped_oldest += 1;
        user_prompt =
            build_diary_user_prompt(&target_date, chrono_view, &topic_labels, total, automated);
    }

    if dropped_oldest > 0 {
        tracing::warn!(
            target_date,
            dropped_oldest,
            limit = LOG_CONTEXT_CHAR_LIMIT,
            "diary input exceeded context budget, dropped oldest sessions to fit"
        );
        eprintln!(
            "경고: diary 입력이 컨텍스트 한도({LOG_CONTEXT_CHAR_LIMIT}자)를 초과 — 오래된 세션 {dropped_oldest}건 제외"
        );
    }

    // LLM 백엔드로 일기 생성
    let body = match generate_log_body(
        &config,
        backend.as_deref(),
        model.as_deref(),
        system_prompt,
        &user_prompt,
        &target_date,
    )
    .await
    {
        Ok(text) => text,
        Err(e) => {
            eprintln!("Log generation failed ({}), using template output", e);
            generate_template(&target_date, &by_project, &topic_labels, total)
        }
    };

    // 결과 출력
    println!("{}", body);

    // vault에 저장 — host suffix 로 머신간 conflict 회피
    let log_dir = config.vault.path.join("log");
    std::fs::create_dir_all(&log_dir)?;
    let host = gethostname::gethostname()
        .to_string_lossy()
        .split('.')
        .next()
        .unwrap_or("unknown")
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let log_path = log_dir.join(format!("{}--{}.md", target_date, host));
    std::fs::write(&log_path, &body)?;
    eprintln!("Saved to {}", log_path.display());

    Ok(())
}

pub fn resolve_backend_name(config: &Config, cli_backend: Option<&str>) -> String {
    cli_backend
        .map(ToOwned::to_owned)
        .or_else(|| config.log.backend.clone())
        .or_else(|| {
            if config.graph.semantic_backend.is_empty() {
                None
            } else {
                Some(config.graph.semantic_backend.clone())
            }
        })
        .unwrap_or_else(|| "ollama".to_string())
}

/// Internal resolution helper exposed for integration tests.
pub fn resolve_log_model(
    config: &Config,
    backend_name: &str,
    cli_model: Option<&str>,
) -> Option<String> {
    if let Some(model) = cli_model {
        return Some(model.to_string());
    }

    if let Some(model) = config.log.model.clone() {
        return Some(model);
    }

    match backend_name {
        "ollama" => Some(config.graph.ollama_model.clone().unwrap_or_else(|| {
            warn_using_default("log.model", LOG_OLLAMA_DEFAULT);
            LOG_OLLAMA_DEFAULT.to_string()
        })),
        "ollama_cloud" => Some(
            config
                .log
                .cloud_model
                .clone()
                .or_else(|| config.graph.cloud_model.clone())
                .unwrap_or_else(|| {
                    warn_using_default("log.cloud_model", LOG_OLLAMA_CLOUD_DEFAULT);
                    LOG_OLLAMA_CLOUD_DEFAULT.to_string()
                }),
        ),
        _ => None,
    }
}

fn resolve_log_api_url<'a>(config: &'a Config, backend_name: &str) -> Option<&'a str> {
    config.log.api_url.as_deref().or(match backend_name {
        "ollama" | "lmstudio" => config.graph.ollama_url.as_deref(),
        "ollama_cloud" => config
            .log
            .cloud_host
            .as_deref()
            .or(config.graph.cloud_host.as_deref()),
        _ => None,
    })
}

fn resolve_log_max_tokens(config: &Config) -> u32 {
    config.log.max_tokens.unwrap_or(4096)
}

fn build_backend_prompt(system_prompt: &str, user_prompt: &str) -> String {
    format!("{system_prompt}\n\n{user_prompt}")
}

pub async fn generate_log_body(
    config: &Config,
    cli_backend: Option<&str>,
    cli_model: Option<&str>,
    system_prompt: &str,
    user_prompt: &str,
    target_date: &str,
) -> Result<String> {
    let backend_name = resolve_backend_name(config, cli_backend);
    let resolved_model = resolve_log_model(config, &backend_name, cli_model);
    eprintln!(
        "Generating work log with {}{} ({})...",
        backend_name,
        resolved_model
            .as_deref()
            .map(|m| format!(":{m}"))
            .unwrap_or_default(),
        target_date
    );

    match backend_name.as_str() {
        "claude" => {
            let model = resolved_model.unwrap_or_else(|| {
                warn_using_default("log.model[claude]", WIKI_CLAUDE_DEFAULT);
                WIKI_CLAUDE_DEFAULT.to_string()
            });
            let backend = ClaudeBackend {
                model,
                vault_path: config.vault.path.clone(),
                timeout_secs: config.wiki.generation_timeout_secs,
            };
            backend
                .generate(&build_backend_prompt(system_prompt, user_prompt))
                .await
        }
        "codex" => {
            let model = resolved_model.unwrap_or_else(|| {
                warn_using_default("log.model[codex]", WIKI_CODEX_DEFAULT);
                WIKI_CODEX_DEFAULT.to_string()
            });
            let backend = CodexBackend {
                model,
                vault_path: config.vault.path.clone(),
                timeout_secs: config.wiki.generation_timeout_secs,
            };
            backend
                .generate(&build_backend_prompt(system_prompt, user_prompt))
                .await
        }
        "haiku" => {
            let backend = HaikuBackend::from_env(
                resolved_model,
                resolve_log_max_tokens(config),
                system_prompt.to_string(),
            )?;
            backend.generate(user_prompt).await
        }
        "ollama" => {
            let backend = OllamaBackend {
                api_url: resolve_log_api_url(config, "ollama")
                    .unwrap_or("http://localhost:11434")
                    .to_string(),
                model: resolved_model.unwrap_or_else(|| LOG_OLLAMA_DEFAULT.to_string()),
                max_tokens: resolve_log_max_tokens(config),
                api_key: None,
                timeout_secs: config.wiki.generation_timeout_secs,
            };
            backend
                .generate(&build_backend_prompt(system_prompt, user_prompt))
                .await
        }
        "ollama_cloud" => {
            let api_url = resolve_log_api_url(config, "ollama_cloud")
                .unwrap_or("https://ollama.com")
                .to_string();
            let model = resolved_model.unwrap_or_else(|| LOG_OLLAMA_DEFAULT.to_string());
            let api_key = config
                .log
                .cloud_api_key
                .clone()
                .or_else(|| config.graph.cloud_api_key.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ollama cloud api key not set \
                         (set `OLLAMA_CLOUD_API_KEY` env or \
                         `[log].cloud_api_key` in config.toml)"
                    )
                })?;
            let backend = OllamaBackend {
                api_url,
                model,
                max_tokens: resolve_log_max_tokens(config),
                api_key: Some(api_key),
                timeout_secs: config.wiki.generation_timeout_secs,
            };
            backend
                .generate(&build_backend_prompt(system_prompt, user_prompt))
                .await
        }
        "lmstudio" => {
            let backend = LmStudioBackend {
                api_url: resolve_log_api_url(config, "lmstudio")
                    .unwrap_or("http://localhost:1234")
                    .to_string(),
                model: resolved_model.unwrap_or_else(|| {
                    warn_using_default("log.model[lmstudio]", GRAPH_LMSTUDIO_DEFAULT);
                    GRAPH_LMSTUDIO_DEFAULT.to_string()
                }),
                max_tokens: resolve_log_max_tokens(config),
                timeout_secs: config.wiki.generation_timeout_secs,
            };
            backend
                .generate(&build_backend_prompt(system_prompt, user_prompt))
                .await
        }
        _ => anyhow::bail!("Unknown log backend: {}", backend_name),
    }
}

/// (project, entry) chronological pair 들을 BTreeMap<project, [entries]> 로 그룹화.
/// BTreeMap 의 알파벳 정렬은 LLM prompt 의 안정적 가독성을 위해 유지하되,
/// 각 project 내 entries 순서는 원본(시간순)을 보존한다.
pub(crate) fn group_entries_by_project(
    entries_chronological: &[(String, String)],
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut by_project: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (proj, entry) in entries_chronological {
        by_project
            .entry(proj.clone())
            .or_default()
            .push(entry.clone());
    }
    by_project
}

/// LLM 에 보낼 diary user_prompt 를 (chronological subset, topics, total/automated 메타) 로부터 빌드.
/// truncation 시 동일 함수로 chrono_view 만 줄여 재호출하면 일관된 출력 보장.
pub(crate) fn build_diary_user_prompt(
    target_date: &str,
    entries_chronological: &[(String, String)],
    topic_labels: &[String],
    total: usize,
    automated: usize,
) -> String {
    let by_project = group_entries_by_project(entries_chronological);
    let mut project_sections = String::new();
    for (proj, entries) in &by_project {
        project_sections.push_str(&format!("### {proj}\n"));
        for e in entries {
            project_sections.push_str(e);
            project_sections.push('\n');
        }
        project_sections.push('\n');
    }
    let topics_line = if topic_labels.is_empty() {
        String::new()
    } else {
        format!("주요 토픽: {}\n\n", topic_labels.join(", "))
    };
    format!(
        "날짜: {target_date}\n총 세션: {total}개 (자동화 제외: {automated}개)\n{topics_line}\
         프로젝트별 작업 내역:\n{project_sections}\n\
         위 내용을 바탕으로 자연스러운 한국어 개발 작업 일지를 작성해주세요.\n\
         형식: 마크다운, 프로젝트별 섹션, 간결하게 (200자 이내)"
    )
}

pub(crate) fn generate_template(
    date: &str,
    by_project: &std::collections::BTreeMap<String, Vec<String>>,
    topics: &[String],
    total: usize,
) -> String {
    let mut out = format!("# {date} 작업 일지\n\n");
    for (proj, entries) in by_project {
        out.push_str(&format!("## {proj}\n"));
        for e in entries {
            out.push_str(e);
            out.push('\n');
        }
        out.push('\n');
    }
    if !topics.is_empty() {
        out.push_str(&format!("**주요 토픽**: {}\n\n", topics.join(", ")));
    }
    out.push_str(&format!("*총 {total}개 세션*\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_generate_template_basic() {
        let mut by_project = BTreeMap::new();
        by_project.insert(
            "seCall".to_string(),
            vec!["- (5턴, 도구:[Edit]) Add feature X".to_string()],
        );
        let topics = vec!["rust".to_string(), "async".to_string()];
        let result = generate_template("2026-04-13", &by_project, &topics, 3);

        assert!(result.contains("# 2026-04-13 작업 일지"));
        assert!(result.contains("## seCall"));
        assert!(result.contains("Add feature X"));
        assert!(result.contains("**주요 토픽**: rust, async"));
        assert!(result.contains("*총 3개 세션*"));
    }

    #[test]
    fn test_generate_template_no_topics() {
        let mut by_project = BTreeMap::new();
        by_project.insert(
            "other".to_string(),
            vec!["- (2턴, 도구:[]) Fix bug".to_string()],
        );
        let result = generate_template("2026-04-12", &by_project, &[], 1);

        assert!(result.contains("# 2026-04-12 작업 일지"));
        assert!(!result.contains("주요 토픽"));
        assert!(result.contains("*총 1개 세션*"));
    }

    #[test]
    fn test_generate_template_multiple_projects() {
        let mut by_project = BTreeMap::new();
        by_project.insert("A".to_string(), vec!["- entry A".to_string()]);
        by_project.insert(
            "B".to_string(),
            vec!["- entry B1".to_string(), "- entry B2".to_string()],
        );
        let result = generate_template("2026-01-01", &by_project, &[], 5);

        // BTreeMap이므로 A가 B보다 먼저 나와야 함
        let a_pos = result.find("## A").unwrap();
        let b_pos = result.find("## B").unwrap();
        assert!(a_pos < b_pos);
        assert!(result.contains("entry B2"));
    }

    #[test]
    fn test_resolve_backend_priority_cli_then_log_then_graph_then_default() {
        let mut config = Config::default();
        config.log.backend = Some("claude".to_string());
        config.graph.semantic_backend = "ollama_cloud".to_string();
        assert_eq!(resolve_backend_name(&config, Some("haiku")), "haiku");

        config.log.backend = None;
        assert_eq!(resolve_backend_name(&config, None), "ollama_cloud");

        config.graph.semantic_backend.clear();
        assert_eq!(resolve_backend_name(&config, None), "ollama");
    }

    #[test]
    fn test_resolve_log_model_ollama_cloud_uses_config_field() {
        let mut config = Config::default();
        config.log.cloud_model = Some("custom:tag".to_string());
        assert_eq!(
            resolve_log_model(&config, "ollama_cloud", None).as_deref(),
            Some("custom:tag")
        );
    }

    #[test]
    fn test_resolve_log_model_ollama_cloud_falls_back_to_graph_cloud_model() {
        let mut config = Config::default();
        config.log.cloud_model = None;
        config.graph.cloud_model = Some("g:m".to_string());
        assert_eq!(
            resolve_log_model(&config, "ollama_cloud", None).as_deref(),
            Some("g:m")
        );
    }

    #[test]
    fn test_resolve_log_model_ollama_cloud_falls_back_to_default() {
        use secall_core::llm::defaults::LOG_OLLAMA_CLOUD_DEFAULT;
        let config = Config::default();
        assert_eq!(
            resolve_log_model(&config, "ollama_cloud", None).as_deref(),
            Some(LOG_OLLAMA_CLOUD_DEFAULT)
        );
    }

    #[test]
    fn test_resolve_log_api_url_ollama_cloud_uses_log_cloud_host_first() {
        let mut config = Config::default();
        config.log.cloud_host = Some("https://custom.ollama.example".to_string());
        config.graph.cloud_host = Some("https://graph.ollama.example".to_string());
        assert_eq!(
            resolve_log_api_url(&config, "ollama_cloud"),
            Some("https://custom.ollama.example"),
            "log.cloud_host should take precedence over graph.cloud_host"
        );
    }

    #[test]
    fn test_resolve_log_api_url_ollama_cloud_falls_back_to_graph_cloud_host() {
        let mut config = Config::default();
        config.log.cloud_host = None;
        config.graph.cloud_host = Some("https://graph.ollama.example".to_string());
        assert_eq!(
            resolve_log_api_url(&config, "ollama_cloud"),
            Some("https://graph.ollama.example"),
            "when log.cloud_host is None, graph.cloud_host should be used"
        );

        // 둘 다 None → None 반환 (caller 에서 default "https://ollama.com" 적용)
        config.graph.cloud_host = None;
        assert_eq!(
            resolve_log_api_url(&config, "ollama_cloud"),
            None,
            "when both are None, resolve_log_api_url returns None"
        );
    }

    #[test]
    fn test_group_entries_by_project_preserves_chronological_order_within_project() {
        // (proj, entry) 시간순 입력
        let chronological = vec![
            ("A".to_string(), "- old A1".to_string()),
            ("B".to_string(), "- old B1".to_string()),
            ("A".to_string(), "- new A2".to_string()),
            ("B".to_string(), "- new B2".to_string()),
        ];
        let by_project = group_entries_by_project(&chronological);
        // 같은 project 내 entries 는 입력 시간순(old → new) 유지
        assert_eq!(by_project["A"], vec!["- old A1", "- new A2"]);
        assert_eq!(by_project["B"], vec!["- old B1", "- new B2"]);
    }

    #[test]
    fn test_build_diary_user_prompt_includes_metadata_and_entries() {
        let chronological = vec![
            ("seCall".to_string(), "- entry-1".to_string()),
            ("other".to_string(), "- entry-2".to_string()),
        ];
        let topics = vec!["rust".to_string()];
        let prompt = build_diary_user_prompt("2026-05-12", &chronological, &topics, 2, 0);
        assert!(prompt.contains("날짜: 2026-05-12"));
        assert!(prompt.contains("총 세션: 2개"));
        assert!(prompt.contains("주요 토픽: rust"));
        assert!(prompt.contains("### seCall"));
        assert!(prompt.contains("### other"));
        assert!(prompt.contains("- entry-1"));
        assert!(prompt.contains("- entry-2"));
    }

    #[test]
    fn test_truncation_drops_oldest_session_first() {
        // P46 Task 04 회귀 — 시간순 가장 오래된 entry 부터 잘려야 한다.
        let chronological = vec![
            (
                "A".to_string(),
                "- OLDEST_unique_marker_for_test".to_string(),
            ),
            ("B".to_string(), "- middle entry".to_string()),
            ("A".to_string(), "- NEWEST_kept".to_string()),
        ];
        let prompt_full = build_diary_user_prompt("2026-05-12", &chronological, &[], 3, 0);
        assert!(prompt_full.contains("OLDEST_unique_marker_for_test"));
        assert!(prompt_full.contains("NEWEST_kept"));

        // 가장 오래된 1건 제거 후 재빌드
        let truncated = &chronological[1..];
        let prompt_trunc = build_diary_user_prompt("2026-05-12", truncated, &[], 3, 0);
        assert!(
            !prompt_trunc.contains("OLDEST_unique_marker_for_test"),
            "oldest session must be dropped after truncation"
        );
        assert!(
            prompt_trunc.contains("NEWEST_kept"),
            "newer entries must be preserved"
        );
        assert!(
            prompt_trunc.contains("middle entry"),
            "middle entry must be preserved"
        );
    }
}
