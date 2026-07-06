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

/// vault 세션 Markdown 본문을 turn 단위로 역파싱한다 (`render_session` 의 역방향).
///
/// 헤딩 규칙:
/// - `## Turn N — Role` → turn (index=N-1, role=Role)
/// - `### Turn N` → 직전 turn 의 role 을 상속 (P49 연속 role 헤더 축약의 역)
///
/// 본문(thinking/tool callout 포함)은 검색 가능한 텍스트 그대로 content 에 보존한다.
/// frontmatter 및 첫 turn 이전 영역(세션 요약 등)은 turn content 에 포함하지 않는다.
///
/// 주의: 렌더링은 lossy 하다 (`escape_dataview_fields` 의 zero-width space 삽입,
/// `collapse_blank_lines`, tool 출력 `TOOL_OUTPUT_MAX_CHARS` 절단). 따라서 복원된
/// turn 은 원본과 byte-identical 하지 않은 **재구성본**이며 FTS/임베딩 검색 목적에
/// 한해 유효하다. CRLF/LF 모두 지원하며 malformed 헤딩은 panic 없이 처리한다.
pub fn parse_session_turns(content: &str) -> crate::error::Result<Vec<super::types::Turn>> {
    use super::types::Turn;

    // CRLF/LF normalize 후 frontmatter 제거
    let normalized = content.replace("\r\n", "\n");
    let body = normalized
        .split_once("\n---\n")
        .map(|(_, b)| b)
        .unwrap_or(normalized.as_str());

    let mut turns: Vec<Turn> = Vec::new();
    // (index, role, content lines)
    let mut cur: Option<(u32, Role, Vec<String>)> = None;
    let mut last_role: Option<Role> = None;
    let mut in_code_block = false;

    fn flush(cur: &mut Option<(u32, Role, Vec<String>)>, turns: &mut Vec<Turn>) {
        if let Some((index, role, lines)) = cur.take() {
            let content = lines.join("\n").trim().to_string();
            turns.push(Turn {
                index,
                role,
                timestamp: None,
                content,
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            });
        }
    }

    for line in body.lines() {
        // fenced code block 토글 — 코드블록 내부의 "## Turn"/"### Turn" 라인을
        // 실제 turn 헤더로 오인하지 않도록 추적한다 (세션 본문이 vault 포맷을
        // 코드블록으로 인용하는 메타 세션에서 오파싱 방지).
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            if let Some((_, _, lines)) = cur.as_mut() {
                lines.push(line.to_string());
            }
            continue;
        }
        if in_code_block {
            if let Some((_, _, lines)) = cur.as_mut() {
                lines.push(line.to_string());
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("## Turn ") {
            match parse_turn_heading_h2(rest) {
                Some((n, role)) => {
                    flush(&mut cur, &mut turns);
                    last_role = Some(role);
                    cur = Some((n.saturating_sub(1), role, Vec::new()));
                }
                // malformed h2 헤딩: 유실 방지 위해 현재 turn 본문으로 취급
                None => {
                    if let Some((_, _, lines)) = cur.as_mut() {
                        lines.push(line.to_string());
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("### Turn ") {
            match parse_turn_heading_h3(rest) {
                Some(n) => {
                    let role = last_role.unwrap_or(Role::User);
                    flush(&mut cur, &mut turns);
                    cur = Some((n.saturating_sub(1), role, Vec::new()));
                }
                None => {
                    if let Some((_, _, lines)) = cur.as_mut() {
                        lines.push(line.to_string());
                    }
                }
            }
        } else if let Some((_, _, lines)) = cur.as_mut() {
            lines.push(line.to_string());
        }
        // 첫 turn 헤딩 이전 라인(요약/헤더)은 무시
    }
    flush(&mut cur, &mut turns);

    Ok(turns)
}

/// `## Turn ` 접두 제거 후 나머지("N — Role" 또는 "N — Role (HH:MM)")에서 (N, Role) 추출.
fn parse_turn_heading_h2(rest: &str) -> Option<(u32, Role)> {
    let (num_part, role_part) = rest.split_once(" — ")?;
    let n: u32 = num_part.trim().parse().ok()?;
    let role = match role_part.split_whitespace().next()? {
        "User" => Role::User,
        "Assistant" => Role::Assistant,
        "System" => Role::System,
        _ => return None,
    };
    Some((n, role))
}

/// `### Turn ` 접두 제거 후 나머지("N" 또는 "N (HH:MM)")에서 N 추출.
fn parse_turn_heading_h3(rest: &str) -> Option<u32> {
    rest.split_whitespace().next()?.parse().ok()
}

const TOOL_OUTPUT_MAX_CHARS: usize = 500;

/// Render a Session to Obsidian-compatible Markdown string
pub fn render_session(session: &Session, tz: chrono_tz::Tz) -> String {
    let mut out = String::new();

    // Frontmatter
    out.push_str("---\n");
    out.push_str("type: session\n");
    out.push_str(&format!("agent: {}\n", yaml_scalar(session.agent.as_str())));
    if let Some(m) = &session.model {
        out.push_str(&format!("model: {}\n", yaml_scalar(m)));
    }
    if let Some(p) = &session.project {
        out.push_str(&format!("project: {}\n", yaml_scalar(p)));
    }
    if let Some(c) = &session.cwd {
        out.push_str(&format!("cwd: {}\n", yaml_scalar(&c.display().to_string())));
    }
    out.push_str(&format!("session_id: {}\n", yaml_scalar(&session.id)));
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
        out.push_str(&format!("host: {}\n", yaml_scalar(host)));
    }
    if let Some(summary) = extract_summary(session) {
        let escaped = escape_yaml_string(&summary);
        out.push_str(&format!("summary: \"{escaped}\"\n"));
    }
    out.push_str("status: raw\n");
    out.push_str(&format!(
        "session_type: {}\n",
        yaml_scalar(&session.session_type)
    ));
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
    // P49 follow-up: `.sessions` dot-prefix 면 obsidian 의 core 인덱서 + 대부분 plugin 이
    // 자동으로 무시한다. 1259+ 세션 md 한 번에 들어올 때의 vault freeze 회피.
    PathBuf::from("raw")
        .join(".sessions")
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
    // char-aware prefix: 멀티바이트 id(예: 파일명 stem "세션백업")를 byte-slice 하면
    // char boundary 위반으로 panic → ingest hot path 전체 abort. char 단위로 자른다.
    let id_prefix: String = session.id.chars().take(8).collect();
    format!("{agent}_{project}_{id_prefix}.md")
}

/// 더블쿼트 YAML 스칼라 내부용 이스케이프. backslash/quote 뿐 아니라 제어문자
/// (개행/탭/ESC 등 C0 및 DEL/C1)도 이스케이프하여 serde_yaml 이 거부하지 않는
/// 유효한 double-quoted scalar 를 만든다.
fn escape_yaml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            // 기타 제어문자(ESC/0x1b 포함 C0, DEL, C1)는 \xNN 로 이스케이프.
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 문자열을 안전한 YAML 스칼라로 렌더링한다. 평범한 값은 그대로 두어 기존 vault
/// 포맷을 유지하고, YAML 메타문자/제어문자가 있으면 double-quote + 이스케이프하여
/// 항상 유효한 YAML 을 보장한다 (frontmatter 왕복 파싱 실패로 인한 silent session
/// drop 방지 — 예: `[archive]`, `@work`, ANSI/제어 바이트 포함 summary).
fn yaml_scalar(s: &str) -> String {
    if is_safe_plain_scalar(s) {
        s.to_string()
    } else {
        format!("\"{}\"", escape_yaml_string(s))
    }
}

/// unquoted(plain) YAML 스칼라로 안전하게 쓸 수 있는지 보수적으로 판정.
/// 안전하지 않으면 호출측에서 double-quote 로 감싼다.
fn is_safe_plain_scalar(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    // 제어문자가 있으면 plain 불가.
    if s.chars().any(|c| c.is_control()) {
        return false;
    }
    let first = s.chars().next().unwrap();
    // 첫 글자가 YAML indicator 면 flow/anchor/tag 등으로 오해될 수 있음.
    if matches!(
        first,
        '-' | '?'
            | ':'
            | ','
            | '['
            | ']'
            | '{'
            | '}'
            | '#'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '\''
            | '"'
            | '%'
            | '@'
            | '`'
            | ' '
    ) {
        return false;
    }
    // 끝 공백, ": "(mapping), " #"(comment), 끝의 ':' 는 plain 에서 위험.
    if s.ends_with(' ') || s.ends_with(':') || s.contains(": ") || s.contains(" #") {
        return false;
    }
    // bool/null 로 오파싱될 수 있는 값은 quote.
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "null" | "~" | "true" | "false" | "yes" | "no" | "on" | "off"
    ) {
        return false;
    }
    true
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
        assert!(path_str.starts_with("raw/.sessions/2026-04-05/"));
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

    // #7 ─ YAML frontmatter injection / silent session drop 회귀 테스트.
    // project `[archive]`(flow seq 로 오파싱), host `@work`(reserved indicator),
    // summary 내 제어문자(ESC) 가 unquoted/미이스케이프되면 serde_yaml 이 frontmatter
    // 전체 파싱에 실패해 세션이 조용히 드랍된다. 렌더 → parse 왕복이 성공해야 한다.
    #[test]
    fn test_frontmatter_roundtrip_adversarial_values() {
        let mut session = make_session(vec![make_turn(Role::User, "hello: world \u{1b}[0m done")]);
        session.project = Some("[archive]".to_string());
        session.host = Some("@work".to_string());
        session.session_type = "type: weird".to_string();
        let md = render_session(&session, chrono_tz::Tz::UTC);

        // 렌더 결과가 다시 파싱되어야 한다 (silent drop 방지).
        let fm = parse_session_frontmatter(&md).expect("adversarial frontmatter must parse");
        assert_eq!(fm.project.as_deref(), Some("[archive]"));
        assert_eq!(fm.host.as_deref(), Some("@work"));
        assert_eq!(fm.session_type.as_deref(), Some("type: weird"));
        // summary 의 콜론과 ESC 제어문자가 그대로 복원되어야 한다.
        assert_eq!(fm.summary.as_deref(), Some("hello: world \u{1b}[0m done"));
    }

    #[test]
    fn test_escape_yaml_string_escapes_control_chars() {
        // ESC(0x1b) 및 개행/탭이 유효한 double-quoted escape 로 변환되어야 한다.
        let escaped = escape_yaml_string("a\u{1b}b\nc\td");
        assert_eq!(escaped, "a\\x1bb\\nc\\td");
        // 왕복: double-quote 로 감싸 serde_yaml 로 파싱하면 원본 복원.
        let yaml = format!("v: \"{}\"", escaped);
        let val: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(val["v"].as_str(), Some("a\u{1b}b\nc\td"));
    }

    // #16 ─ 멀티바이트 id(예: 파일명 stem "세션백업메모1234") 를 byte-slice 하면
    // char boundary 위반으로 panic 하여 ingest 전체가 abort 된다. char-aware prefix
    // 는 panic 없이 앞 8 char 를 취해야 한다.
    #[test]
    fn test_session_filename_multibyte_id_no_panic() {
        let mut session = make_session(vec![]);
        // 10 chars(각 한글 3바이트) → byte index 8 은 char boundary 아님.
        session.id = "세션백업메모1234".to_string();
        let path = session_vault_path(&session, chrono_tz::Tz::UTC);
        let s = path.to_string_lossy();
        // 앞 8 char prefix: "세션백업메모12"
        assert!(
            s.contains("세션백업메모12"),
            "filename must contain char-aware 8-char id prefix: {s}"
        );
        assert!(s.ends_with(".md"));
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

    // ─── parse_session_turns (역파서) ──────────────────────────────────────────

    #[test]
    fn parse_h2_role_and_index() {
        // `## Turn N — Role` 에서 role 과 index(0-based) 복원
        let md = "---\nsession_id: x\n---\n\n## Turn 1 — User\n\nhello\n\n## Turn 2 — Assistant\n\nhi there\n";
        let turns = parse_session_turns(md).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].index, 0);
        assert_eq!(turns[0].role, Role::User);
        assert_eq!(turns[0].content, "hello");
        assert_eq!(turns[1].index, 1);
        assert_eq!(turns[1].role, Role::Assistant);
        assert_eq!(turns[1].content, "hi there");
    }

    #[test]
    fn parse_h3_inherits_previous_role() {
        // 연속 role 의 `### Turn N` 은 직전 role 을 상속
        let md = "---\ns: 1\n---\n\n## Turn 2 — Assistant\n\nfirst\n\n### Turn 3\n\nsecond\n";
        let turns = parse_session_turns(md).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[1].index, 2);
        assert_eq!(turns[1].role, Role::Assistant, "h3 must inherit prior role");
        assert_eq!(turns[1].content, "second");
    }

    #[test]
    fn parse_supports_crlf() {
        let md = "---\r\ns: 1\r\n---\r\n\r\n## Turn 1 — User\r\n\r\nline a\r\nline b\r\n";
        let turns = parse_session_turns(md).unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].role, Role::User);
        assert_eq!(turns[0].content, "line a\nline b");
    }

    #[test]
    fn parse_excludes_frontmatter_and_summary() {
        // 첫 `## Turn` 이전 영역(요약 blockquote 등)은 turn content 에 포함되지 않음
        let md = "---\ns: 1\n---\n\n> **프로젝트**: p | **브랜치**: main\n\n## Turn 1 — User\n\nreal content\n";
        let turns = parse_session_turns(md).unwrap();
        assert_eq!(turns.len(), 1);
        assert!(!turns[0].content.contains("프로젝트"));
        assert_eq!(turns[0].content, "real content");
    }

    #[test]
    fn parse_malformed_heading_no_panic() {
        // `## Turn abc — User` 는 숫자 파싱 실패 → panic 없이 본문 취급
        let md = "---\ns: 1\n---\n\n## Turn 1 — User\n\nbody\n\n## Turn abc — User\ntrailing\n";
        let turns = parse_session_turns(md).unwrap();
        assert_eq!(turns.len(), 1, "malformed heading must not create a turn");
        assert!(turns[0].content.contains("body"));
    }

    #[test]
    fn parse_roundtrip_from_render() {
        // render_session → parse_session_turns 왕복: 개수/role/index 보존
        let turns = vec![
            make_turn(Role::User, "question one"),
            make_turn(Role::Assistant, "answer one"),
            make_turn(Role::Assistant, "answer one cont"),
            make_turn(Role::User, "question two"),
        ];
        // make_turn 은 index 를 세팅하지 않으므로 명시적으로 부여
        let turns: Vec<Turn> = turns
            .into_iter()
            .enumerate()
            .map(|(i, mut t)| {
                t.index = i as u32;
                t
            })
            .collect();
        let session = make_session(turns.clone());
        let md = render_session(&session, chrono_tz::Tz::UTC);
        let parsed = parse_session_turns(&md).unwrap();
        assert_eq!(parsed.len(), turns.len());
        for (orig, got) in turns.iter().zip(parsed.iter()) {
            assert_eq!(orig.index, got.index);
            assert_eq!(orig.role, got.role);
            assert!(got.content.contains(&orig.content));
        }
    }
}
