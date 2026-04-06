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
}


/// vault 마크다운 파일에서 frontmatter YAML을 파싱.
pub fn parse_session_frontmatter(content: &str) -> crate::error::Result<SessionFrontmatter> {
    let fm = content
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---"))
        .map(|(fm, _)| fm)
        .ok_or_else(|| crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: anyhow::anyhow!("no frontmatter found"),
        })?;

    let parsed: SessionFrontmatter = serde_yaml::from_str(fm).map_err(|e| {
        crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: e.into(),
        }
    })?;
    Ok(parsed)
}

/// frontmatter 이후의 본문 텍스트 추출 (턴 내용).
pub fn extract_body_text(content: &str) -> String {
    content
        .split_once("\n---\n")
        .map(|(_, body)| body.split_once('\n').map(|(_, rest)| rest).unwrap_or(body))
        .unwrap_or("")
        .to_string()
}

const TOOL_OUTPUT_MAX_CHARS: usize = 500;

/// Render a Session to Obsidian-compatible Markdown string
pub fn render_session(session: &Session) -> String {
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
        session.start_time.format("%Y-%m-%d")
    ));
    out.push_str(&format!(
        "start_time: \"{}\"\n",
        session.start_time.format("%Y-%m-%dT%H:%M:%S+00:00")
    ));
    if let Some(end) = session.end_time {
        out.push_str(&format!(
            "end_time: \"{}\"\n",
            end.format("%Y-%m-%dT%H:%M:%S+00:00")
        ));
    }
    out.push_str(&format!("turns: {}\n", session.turns.len()));
    out.push_str(&format!("tokens_in: {}\n", session.total_tokens.input));
    out.push_str(&format!("tokens_out: {}\n", session.total_tokens.output));

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
    out.push_str("status: raw\n");
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
    let start_str = session.start_time.format("%H:%M").to_string();
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
    for turn in &session.turns {
        let role_str = match turn.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };

        let ts_str = turn
            .timestamp
            .map(|t| format!(" ({})", t.format("%H:%M")))
            .unwrap_or_default();

        out.push_str(&format!(
            "## Turn {} — {}{}\n\n",
            turn.index + 1,
            role_str,
            ts_str
        ));

        // Thinking block
        if let Some(thinking) = &turn.thinking {
            out.push_str("> [!thinking]- Thinking\n");
            for line in thinking.lines() {
                out.push_str(&format!("> {}\n", line));
            }
            out.push('\n');
        }

        // Main content
        if !turn.content.is_empty() {
            // Collapse repeated blank lines
            let cleaned = collapse_blank_lines(&turn.content);
            out.push_str(&cleaned);
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
pub fn session_vault_path(session: &Session) -> PathBuf {
    let date = session.start_time.format("%Y-%m-%d").to_string();
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

fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars].iter().collect();
        format!("{}...", truncated)
    }
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
        }
    }

    #[test]
    fn test_render_basic_frontmatter() {
        let session = make_session(vec![]);
        let md = render_session(&session);
        assert!(md.starts_with("---\n"));
        assert!(md.contains("type: session\n"));
        assert!(md.contains("agent: claude-code\n"));
        assert!(md.contains("session_id: a1b2c3d4"));
        assert!(md.contains("project: seCall\n"));
        assert!(md.contains("model: claude-opus-4-6\n"));
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
        let md = render_session(&session);
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
        let md = render_session(&session);
        assert!(md.contains("> [!thinking]- Thinking"));
        assert!(md.contains("Internal reasoning"));
    }

    #[test]
    fn test_render_empty_session() {
        let session = make_session(vec![]);
        let md = render_session(&session);
        // Should still have valid frontmatter + title
        assert!(md.contains("---"));
        assert!(md.contains("# claude-code 세션: seCall"));
    }

    #[test]
    fn test_session_vault_path() {
        let session = make_session(vec![]);
        let path = session_vault_path(&session);
        let path_str = path.to_string_lossy();
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
        let md = render_session(&session);
        // Should be truncated to 500+3 (for "...")
        assert!(md.contains("..."));
    }

    #[test]
    fn test_frontmatter_yaml_valid() {
        let session = make_session(vec![]);
        let md = render_session(&session);
        // Extract frontmatter
        let after_first = &md[4..]; // skip "---\n"
        let end = after_first.find("---\n").unwrap();
        let frontmatter = &after_first[..end];
        // Basic checks: no unescaped special chars that break YAML
        assert!(!frontmatter.contains(":\n:")); // no double colon issues
    }
}
