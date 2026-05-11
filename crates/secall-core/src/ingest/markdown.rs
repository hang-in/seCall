use std::path::PathBuf;

use super::types::{Action, Role, Session};

// ─── Vault reverse-parsing ────────────────────────────────────────────────────

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct SessionFrontmatter {
    pub session_id: String,
    pub agent: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<String>,
    pub date: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub turns: Option<u32>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub tools_used: Option<Vec<String>>,
    pub host: Option<String>,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub session_type: Option<String>,
    /// P45 — vault SSOT. archived=true 면 DB 의 is_archived 도 갱신.
    pub archived: Option<bool>,
    /// RFC3339 string.
    pub archived_at: Option<String>,
}

/// vault 마크다운 파일에서 frontmatter YAML을 파싱.
pub fn parse_session_frontmatter(content: &str) -> crate::error::Result<SessionFrontmatter> {
    let normalized = content.replace("\r\n", "\n");
    let fm = normalized
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---"))
        .map(|(fm, _)| fm)
        .ok_or_else(|| crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: anyhow::anyhow!("no frontmatter found"),
        })?;

    let parsed: SessionFrontmatter =
        serde_yaml::from_str(fm).map_err(|e| crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: e.into(),
        })?;
    Ok(parsed)
}

/// frontmatter 이후의 본문 텍스트 추출 (턴 내용).
pub fn extract_body_text(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .split_once("\n---\n")
        .map(|(_, body)| body.split_once('\n').map(|(_, rest)| rest).unwrap_or(body))
        .unwrap_or("")
        .to_string()
}

const TOOL_OUTPUT_MAX_CHARS: usize = 500;

/// Render a Session to Obsidian-compatible Markdown string
pub fn render_session(session: &Session, tz: chrono_tz::Tz) -> String {
    let mut out = String::new();

    // Frontmatter
    out.push_str("---\n");
    out.push_str("type: session\n");
    out.push_str(&format!("agent: {}\n", session.agent.as_str()));
    if let Some(m) = &session.model {
        out.push_str(&format!("model: {}\n", m));
    }
    if let Some(p) = &session.project {
        out.push_str(&format!("project: {}\n", p));
    }
    if let Some(c) = &session.cwd {
        out.push_str(&format!("cwd: {}\n", c.display()));
    }
    out.push_str(&format!("session_id: {}\n", session.id));
    out.push_str(&format!(
        "date: {}\n",
        session.start_time.with_timezone(&tz).format("%Y-%m-%d")
    ));
    out.push_str(&format!(
        "start_time: \"{}\"\n",
        session
            .start_time
            .with_timezone(&tz)
            .format("%Y-%m-%dT%H:%M:%S%:z")
    ));
    if let Some(end) = session.end_time {
        out.push_str(&format!(
            "end_time: \"{}\"\n",
            end.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%:z")
        ));
    }
    out.push_str(&format!("turns: {}\n", session.turns.len()));
    out.push_str(&format!("tokens_in: {}\n", session.total_tokens.input));
    out.push_str(&format!("tokens_out: {}\n", session.total_tokens.output));

    if session.archived {
        out.push_str("archived: true\n");
        if let Some(at) = session.archived_at {
            out.push_str(&format!(
                "archived_at: \"{}\"\n",
                at.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%:z")
            ));
        }
    }

    // Collect unique tool names
    let mut tools_used: Vec<String> = Vec::new();
    for turn in &session.turns {
        for action in &turn.actions {
            if let Action::ToolUse { name, .. } = action {
                if !tools_used.contains(name) {
                    tools_used.push(name.clone());
                }
            }
        }
    }
    out.push_str(&format!("tools_used: [{}]\n", tools_used.join(", ")));
    if let Some(host) = &session.host {
        out.push_str(&format!("host: {host}\n"));
    }
    if let Some(summary) = extract_summary(session) {
        let escaped = escape_yaml_string(&summary);
        out.push_str(&format!("summary: \"{escaped}\"\n"));
    }
    out.push_str("status: raw\n");
    out.push_str(&format!("session_type: {}\n", session.session_type));
    out.push_str("---\n\n");

    // Title
    let project = session.project.as_deref().unwrap_or("unknown");
    out.push_str(&format!(
        "# {} 세션: {}\n\n",
        session.agent.as_str(),
        project
    ));

    // Summary line
    let branch = session.git_branch.as_deref().unwrap_or("-");
    let start_str = session
        .start_time
        .with_timezone(&tz)
        .format("%H:%M")
        .to_string();
    let time_summary = if let Some(end) = session.end_time {
        let duration = end.signed_duration_since(session.start_time);
        let mins = duration.num_minutes();
        if mins >= 60 {
            format!("{} ({}h {}m)", start_str, mins / 60, mins % 60)
        } else {
            format!("{} ({}m)", start_str, mins)
        }
    } else {
        start_str
    };

    out.push_str(&format!(
        "> **프로젝트**: {} | **브랜치**: {} | **시간**: {}\n\n",
        project, branch, time_summary
    ));

    // Turns
    //
    // P49: 같은 role 의 연속 turn 들은 h3 + role 명 생략으로 헤더 노이즈를 줄인다.
    // (claude-code 의 한 응답이 tool_use 마다 별도 turn 으로 쪼개지면서 Assistant 헤더가
    // 5-10번 연달아 나오던 web UI 가독성 이슈)
    let mut last_role: Option<Role> = None;
    for turn in &session.turns {
        let role_str = match turn.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };

        let ts_str = turn
            .timestamp
            .map(|t| format!(" ({})", t.with_timezone(&tz).format("%H:%M")))
            .unwrap_or_default();

        let same_role_as_prev = last_role.as_ref() == Some(&turn.role);
        if same_role_as_prev {
            out.push_str(&format!("### Turn {}{}\n\n", turn.index + 1, ts_str));
        } else {
            out.push_str(&format!(
                "## Turn {} — {}{}\n\n",
                turn.index + 1,
                role_str,
                ts_str
            ));
        }
        last_role = Some(turn.role);

        // Thinking block
        if let Some(thinking) = &turn.thinking {
            let escaped = escape_dataview_fields(thinking);
            out.push_str("> [!thinking]- Thinking\n");
            for line in escaped.lines() {
                out.push_str(&format!("> {}\n", line));
            }
            out.push('\n');
        }

        // Main content
        if !turn.content.is_empty() {
            // Collapse repeated blank lines, then escape Dataview :: patterns
            let cleaned = collapse_blank_lines(&turn.content);
            let escaped = escape_dataview_fields(&cleaned);
            out.push_str(&escaped);
            out.push_str("\n\n");
        }

        // Tool actions
        for action in &turn.actions {
            match action {
                Action::ToolUse {
                    name,
                    input_summary,
                    output_summary,
                    ..
                } => {
                    out.push_str(&format!("> [!tool]- {}\n", name));
                    if !input_summary.is_empty() {
                        out.push_str("> ```\n");
                        for line in input_summary.lines() {
                            out.push_str(&format!("> {}\n", line));
                        }
                        out.push_str("> ```\n");
                    }
                    if !output_summary.is_empty() {
                        let truncated = truncate_str(output_summary, TOOL_OUTPUT_MAX_CHARS);
                        out.push_str("> **Output:**\n");
                        out.push_str("> ```\n");
                        for line in truncated.lines() {
                            out.push_str(&format!("> {}\n", line));
                        }
                        out.push_str("> ```\n");
                    }
                    out.push('\n');
                }
                Action::FileEdit { path } => {
                    out.push_str(&format!("> [!tool]- Edit `{}`\n\n", path));
                }
                Action::Command { cmd, exit_code } => {
                    out.push_str(&format!("> [!tool]- Command\n> ```\n> {}\n> ```\n", cmd));
                    if let Some(code) = exit_code {
                        out.push_str(&format!("> Exit: {}\n", code));
                    }
                    out.push('\n');
                }
            }
        }
    }

    out
}

/// Generate the vault-relative path for a session file
pub fn session_vault_path(session: &Session, tz: chrono_tz::Tz) -> PathBuf {
    let date = session
        .start_time
        .with_timezone(&tz)
        .format("%Y-%m-%d")
        .to_string();
    let filename = session_filename(session);
    PathBuf::from("raw")
        .join("sessions")
        .join(date)
        .join(filename)
}

fn session_filename(session: &Session) -> String {
    let agent = session.agent.as_str();
    let raw_project = session.project.as_deref().unwrap_or("unknown");
    let sanitized: String = raw_project
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == '\0' {
                '_'
            } else {
                c
            }
        })
        .collect();
    let project = if sanitized.starts_with('.') {
        format!("_{sanitized}")
    } else {
        sanitized
    };
    let project = project.as_str();
    let id_prefix = if session.id.len() >= 8 {
        &session.id[..8]
    } else {
        &session.id
    };
    format!("{agent}_{project}_{id_prefix}.md")
}

fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 세션의 첫 User 턴에서 비어있지 않은 첫 줄을 80자로 truncate하여 반환.
pub(crate) fn extract_summary(session: &super::types::Session) -> Option<String> {
    let first_user_turn = session
        .turns
        .iter()
        .find(|t| t.role == super::types::Role::User)?;
    let first_line = first_user_turn
        .content
        .lines()
        .find(|l| !l.trim().is_empty())?;
    let trimmed = first_line.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some(truncate_str(&trimmed, 80))
}

/// vault MD 본문에서 첫 User 턴의 실질적 첫 줄을 summary로 추출.
pub fn extract_summary_from_body(content: &str) -> Option<String> {
    // frontmatter 이후 본문
    let body = content
        .split_once("\n---\n")
        .map(|(_, b)| b)
        .unwrap_or(content);

    // "## Turn N — User" 패턴의 첫 번째 섹션 찾기
    let mut in_user_section = false;
    for line in body.lines() {
        if line.starts_with("## Turn ") && line.contains("— User") {
            in_user_section = true;
            continue;
        }
        if in_user_section {
            // 다음 ## 헤더가 나오면 종료
            if line.starts_with("## ") {
                break;
            }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                return Some(truncate_str(trimmed, 80));
            }
        }
    }
    None
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars].iter().collect();
        format!("{}...", truncated)
    }
}

/// Dataview inline field 방지: fenced code block / inline code 밖의 `::` 사이에
/// zero-width space를 삽입하여 Dataview 파서가 인식하지 못하게 한다.
fn escape_dataview_fields(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_fenced = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fenced = !in_fenced;
            result.push_str(line);
            result.push('\n');
            continue;
        }
        if in_fenced || !line.contains("::") {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        // Inline code 구간을 보존하면서 바깥의 :: 만 이스케이프
        let mut escaped = String::with_capacity(line.len());
        let mut in_inline = false;
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;
        while i < len {
            if chars[i] == '`' {
                in_inline = !in_inline;
                escaped.push('`');
                i += 1;
            } else if !in_inline && chars[i] == ':' && i + 1 < len && chars[i + 1] == ':' {
                escaped.push(':');
                escaped.push('\u{200B}'); // zero-width space
                escaped.push(':');
                i += 2;
            } else {
                escaped.push(chars[i]);
                i += 1;
            }
        }
        result.push_str(&escaped);
        result.push('\n');
    }
    result.trim_end_matches('\n').to_string()
}

fn collapse_blank_lines(text: &str) -> String {
    let mut result = String::new();
    let mut last_was_empty = false;
    for line in text.lines() {
        let is_empty = line.trim().is_empty();
        if is_empty && last_was_empty {
            continue;
        }
        result.push_str(line);
        result.push('\n');
        last_was_empty = is_empty;
    }
    // Trim trailing newlines
    result.trim_end_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use chrono::TimeZone;

    fn make_session(turns: Vec<Turn>) -> Session {
        Session {
            id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(),
            agent: AgentKind::ClaudeCode,
            model: Some("claude-opus-4-6".to_string()),
            project: Some("seCall".to_string()),
            cwd: Some(PathBuf::from("/Users/user/seCall")),
            git_branch: Some("main".to_string()),
            host: None,
            start_time: chrono::Utc.with_ymd_and_hms(2026, 4, 5, 5, 30, 0).unwrap(),
            end_time: Some(chrono::Utc.with_ymd_and_hms(2026, 4, 5, 6, 45, 0).unwrap()),
            turns,
            total_tokens: TokenUsage {
                input: 45000,
                output: 12000,
                cached: 0,
            },
            session_type: "interactive".to_string(),
            archived: false,
            archived_at: None,
        }
    }

    #[test]
    fn test_render_basic_frontmatter() {
        let session = make_session(vec![]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.starts_with("---\n"));
        assert!(md.contains("type: session\n"));
        assert!(md.contains("agent: claude-code\n"));
        assert!(md.contains("session_id: a1b2c3d4"));
        assert!(md.contains("project: seCall\n"));
        assert!(md.contains("model: claude-opus-4-6\n"));
    }

    #[test]
    fn test_render_session_archived_false_omits_field() {
        let session = make_session(vec![]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(
            !md.contains("archived:"),
            "archived: should not appear when false"
        );
    }

    #[test]
    fn test_render_session_archived_true_includes_field() {
        let mut session = make_session(vec![]);
        session.archived = true;
        session.archived_at = Some(chrono::Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap());
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains("\narchived: true\n"), "archived: true missing");
        assert!(md.contains("archived_at:"), "archived_at missing");
    }

    #[test]
    fn test_render_tool_callout() {
        let turns = vec![
            Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: "Run ls".to_string(),
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 1,
                role: Role::Assistant,
                timestamp: None,
                content: "Running ls now".to_string(),
                actions: vec![Action::ToolUse {
                    name: "Bash".to_string(),
                    input_summary: "ls -la".to_string(),
                    output_summary: "file1.txt\nfile2.txt".to_string(),
                    tool_use_id: None,
                }],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
        ];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains("> [!tool]- Bash"));
        assert!(md.contains("ls -la"));
        assert!(md.contains("file1.txt"));
    }

    #[test]
    fn test_render_thinking_callout() {
        let turns = vec![Turn {
            index: 0,
            role: Role::Assistant,
            timestamp: None,
            content: "Answer".to_string(),
            actions: Vec::new(),
            tokens: None,
            thinking: Some("Internal reasoning".to_string()),
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains("> [!thinking]- Thinking"));
        assert!(md.contains("Internal reasoning"));
    }

    #[test]
    fn test_render_empty_session() {
        let session = make_session(vec![]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        // Should still have valid frontmatter + title
        assert!(md.contains("---"));
        assert!(md.contains("# claude-code 세션: seCall"));
    }

    #[test]
    fn test_session_vault_path() {
        let session = make_session(vec![]);
        let path = session_vault_path(&session, chrono_tz::Tz::UTC);
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert!(path_str.starts_with("raw/sessions/2026-04-05/"));
        assert!(path_str.contains("claude-code_seCall_a1b2c3d"));
        assert!(path_str.ends_with(".md"));
    }

    #[test]
    fn test_tool_output_truncation() {
        let long_output = "x".repeat(1000);
        let turns = vec![Turn {
            index: 0,
            role: Role::Assistant,
            timestamp: None,
            content: String::new(),
            actions: vec![Action::ToolUse {
                name: "Bash".to_string(),
                input_summary: "cmd".to_string(),
                output_summary: long_output,
                tool_use_id: None,
            }],
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        // Should be truncated to 500+3 (for "...")
        assert!(md.contains("..."));
    }

    #[test]
    fn test_frontmatter_yaml_valid() {
        let session = make_session(vec![]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        // Extract frontmatter
        let after_first = &md[4..]; // skip "---\n"
        let end = after_first.find("---\n").unwrap();
        let frontmatter = &after_first[..end];
        // Basic checks: no unescaped special chars that break YAML
        assert!(!frontmatter.contains(":\n:")); // no double colon issues
    }

    fn make_turn(role: Role, content: &str) -> Turn {
        Turn {
            index: 0,
            role,
            timestamp: None,
            content: content.to_string(),
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }
    }

    #[test]
    fn test_summary_from_first_user_turn() {
        let session = make_session(vec![make_turn(Role::User, "세션 요약 기능 추가")]);
        let summary = extract_summary(&session);
        assert_eq!(summary, Some("세션 요약 기능 추가".to_string()));
    }

    #[test]
    fn test_summary_skips_empty_lines() {
        let session = make_session(vec![make_turn(Role::User, "\n\n실제 내용")]);
        let summary = extract_summary(&session);
        assert_eq!(summary, Some("실제 내용".to_string()));
    }

    #[test]
    fn test_summary_truncation() {
        let long_content = "a".repeat(100);
        let session = make_session(vec![make_turn(Role::User, &long_content)]);
        let summary = extract_summary(&session);
        let s = summary.unwrap();
        // 80 chars + "..."
        assert_eq!(s.len(), 83);
        assert!(s.ends_with("..."));
    }

    #[test]
    fn test_summary_none_when_no_user_turn() {
        let session = make_session(vec![make_turn(Role::Assistant, "응답 내용")]);
        let summary = extract_summary(&session);
        assert_eq!(summary, None);
    }

    #[test]
    fn test_summary_yaml_escape() {
        let session = make_session(vec![make_turn(
            Role::User,
            r#"say "hello" and \ backslash"#,
        )]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains(r#"summary: "say \"hello\" and \\ backslash""#));
    }

    #[test]
    fn test_summary_in_frontmatter() {
        let session = make_session(vec![make_turn(Role::User, "첫 번째 사용자 메시지")]);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains("summary: \"첫 번째 사용자 메시지\""));
        // summary가 status 전에 위치하는지 확인
        let summary_pos = md.find("summary:").unwrap();
        let status_pos = md.find("status: raw").unwrap();
        assert!(summary_pos < status_pos);
    }

    #[test]
    fn test_escape_dataview_plain_text() {
        let input = "key:: value\nnormal line";
        let escaped = escape_dataview_fields(input);
        assert!(escaped.contains("key:\u{200B}: value"));
        assert!(escaped.contains("normal line"));
    }

    #[test]
    fn test_escape_dataview_preserves_fenced_code() {
        let input = "before:: x\n```\ninside:: y\n```\nafter:: z";
        let escaped = escape_dataview_fields(input);
        // fenced block 안은 그대로
        assert!(escaped.contains("inside:: y"));
        // 밖은 이스케이프
        assert!(escaped.contains("before:\u{200B}: x"));
        assert!(escaped.contains("after:\u{200B}: z"));
    }

    #[test]
    fn test_escape_dataview_preserves_inline_code() {
        let input = "see `key:: value` here and bare:: field";
        let escaped = escape_dataview_fields(input);
        // inline code 안은 그대로
        assert!(escaped.contains("`key:: value`"));
        // 밖은 이스케이프
        assert!(escaped.contains("bare:\u{200B}: field"));
    }

    #[test]
    fn test_escape_dataview_no_colons() {
        let input = "no colons here";
        let escaped = escape_dataview_fields(input);
        assert_eq!(escaped, input);
    }

    #[test]
    fn test_render_session_with_kst_timezone() {
        let session = make_session(vec![]);
        let tz: chrono_tz::Tz = "Asia/Seoul".parse().unwrap();
        let md = render_session(&session, tz);

        // UTC 05:30 → KST 14:30
        assert!(md.contains("date: 2026-04-05"));
        assert!(md.contains("+09:00"));
        assert!(md.contains("14:30"));
    }

    #[test]
    fn test_render_session_utc_default() {
        let session = make_session(vec![]);
        let md = render_session(&session, chrono_tz::Tz::UTC);

        // 기존 동작과 동일
        assert!(md.contains("date: 2026-04-05"));
        assert!(md.contains("+00:00"));
        assert!(md.contains("05:30"));
    }

    #[test]
    fn test_vault_path_uses_timezone_date() {
        let session = make_session(vec![]);
        let path_utc = session_vault_path(&session, chrono_tz::Tz::UTC);
        assert!(path_utc.to_string_lossy().contains("2026-04-05"));
    }

    #[test]
    fn test_vault_path_date_crosses_midnight() {
        // UTC 2026-04-05T15:30 → KST 2026-04-06T00:30 (날짜 변경!)
        let mut session = make_session(vec![]);
        session.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 5, 15, 30, 0).unwrap();
        let tz: chrono_tz::Tz = "Asia/Seoul".parse().unwrap();
        let path = session_vault_path(&session, tz);
        assert!(path.to_string_lossy().contains("2026-04-06"));
    }

    #[test]
    fn test_render_escapes_dataview_in_content() {
        let turns = vec![make_turn(Role::User, "field:: value in conversation")];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        // body에서 :: 가 이스케이프되어야 함
        assert!(md.contains("field:\u{200B}: value"));
        // frontmatter의 :: 는 이 테스트와 무관
    }

    #[test]
    fn test_parse_session_frontmatter_with_archived() {
        let content = "---\nsession_id: test-id\nagent: claude-code\ndate: 2026-05-12\nstart_time: \"2026-05-12T10:00:00+00:00\"\narchived: true\narchived_at: \"2026-05-12T15:00:00Z\"\n---\n\nBody text.";
        let fm = parse_session_frontmatter(content).unwrap();
        assert_eq!(fm.archived, Some(true));
        assert_eq!(fm.archived_at.as_deref(), Some("2026-05-12T15:00:00Z"));
    }

    #[test]
    fn test_parse_session_frontmatter_without_archived_defaults_to_none() {
        let content = "---\nsession_id: test-id\nagent: claude-code\ndate: 2026-05-12\nstart_time: \"2026-05-12T10:00:00+00:00\"\n---\n\nBody text.";
        let fm = parse_session_frontmatter(content).unwrap();
        assert_eq!(fm.archived, None);
        assert_eq!(fm.archived_at, None);
    }

    // P49 ─ 같은 role 연속 turn 헤더 강등 (h2 → h3) 회귀 테스트
    #[test]
    fn test_consecutive_same_role_turns_use_h3_header() {
        let turns = vec![
            Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: "Q".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 1,
                role: Role::Assistant,
                timestamp: None,
                content: "first reply".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 2,
                role: Role::Assistant,
                timestamp: None,
                content: "more reply".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 3,
                role: Role::Assistant,
                timestamp: None,
                content: "tail".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
        ];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        assert!(md.contains("## Turn 1 — User"), "user turn keeps h2");
        assert!(
            md.contains("## Turn 2 — Assistant"),
            "first assistant turn uses h2 + role"
        );
        assert!(
            md.contains("### Turn 3"),
            "second consecutive assistant turn uses h3"
        );
        assert!(
            md.contains("### Turn 4"),
            "third consecutive assistant turn uses h3"
        );
        // role 명이 h3 헤더에는 등장하지 않음
        assert!(
            !md.contains("### Turn 3 — Assistant"),
            "h3 should omit role name"
        );
    }

    #[test]
    fn test_role_change_resets_to_h2() {
        let turns = vec![
            Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: "Q1".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 1,
                role: Role::Assistant,
                timestamp: None,
                content: "A1".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 2,
                role: Role::Assistant,
                timestamp: None,
                content: "A1-cont".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
            Turn {
                index: 3,
                role: Role::User,
                timestamp: None,
                content: "Q2".to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            },
        ];
        let session = make_session(turns);
        let md = render_session(&session, chrono_tz::Tz::UTC);
        // role 이 다시 User 로 바뀌면 h2 + role 명 재출현
        assert!(
            md.contains("## Turn 4 — User"),
            "role change back to User must emit h2 again"
        );
    }
}
