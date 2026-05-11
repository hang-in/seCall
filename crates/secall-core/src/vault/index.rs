use std::path::Path;

use anyhow::Result;

use crate::ingest::Session;

/// 인덱스 한 줄 엔트리 생성 (I/O 없음)
pub(crate) fn build_entry_line(
    link_path: &str,
    title: &str,
    turns: usize,
    agent: &str,
    time_str: &str,
) -> String {
    format!(
        "- [[{}|{}]] — {}턴, {}, {}\n",
        link_path, title, turns, agent, time_str
    )
}

/// content에 entry를 삽입하거나 append.
/// - "## Sessions\n\n" 헤더 있으면 직후 삽입 (최신 항목이 맨 위)
/// - 헤더 없으면 content 끝에 "\n## Sessions\n\n{entry}" 추가
pub(crate) fn insert_into_content(content: &mut String, entry: &str) {
    if let Some(pos) = content.find("## Sessions\n\n") {
        let insert_at = pos + "## Sessions\n\n".len();
        content.insert_str(insert_at, entry);
    } else {
        content.push_str("\n## Sessions\n\n");
        content.push_str(entry);
    }
}

pub fn update_index(
    vault_path: &Path,
    session: &Session,
    md_path: &Path,
    tz: chrono_tz::Tz,
) -> Result<()> {
    let index_path = vault_path.join("index.md");
    let mut content = if index_path.exists() {
        std::fs::read_to_string(&index_path)?
    } else {
        "---\ntype: index\n---\n\n# seCall Index\n\n".to_string()
    };

    // Extract first user turn for title
    let title = session
        .turns
        .iter()
        .find(|t| t.role == crate::ingest::Role::User)
        .map(|t| {
            let s: String = t.content.chars().take(50).collect();
            if t.content.len() > 50 {
                format!("{}...", s)
            } else {
                s
            }
        })
        .unwrap_or_else(|| "Untitled Session".to_string());

    // Build the vault-relative link path (without .md extension for Obsidian)
    let link_path = md_path
        .to_string_lossy()
        .trim_end_matches(".md")
        .to_string();

    let agent = session.agent.as_str();
    let _project = session.project.as_deref().unwrap_or("unknown");
    let time_str = session
        .start_time
        .with_timezone(&tz)
        .format("%H:%M")
        .to_string();
    let turns = session.turns.len();

    let new_entry = build_entry_line(&link_path, &title, turns, agent, &time_str);
    insert_into_content(&mut content, &new_entry);

    std::fs::write(&index_path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_entry_line_format() {
        let line = build_entry_line(
            "raw/.sessions/2026-04-01/abc",
            "디버깅 세션",
            5,
            "claude-code",
            "14:30",
        );
        assert_eq!(
            line,
            "- [[raw/.sessions/2026-04-01/abc|디버깅 세션]] — 5턴, claude-code, 14:30\n"
        );
    }

    #[test]
    fn test_insert_with_header() {
        let mut content =
            "---\ntype: index\n---\n\n# seCall Index\n\n## Sessions\n\n- [[old|old entry]] — 3턴, codex, 10:00\n"
                .to_string();
        insert_into_content(
            &mut content,
            "- [[new|new entry]] — 5턴, claude-code, 14:30\n",
        );
        let sessions_pos = content.find("## Sessions\n\n").unwrap();
        let after_header = &content[sessions_pos + "## Sessions\n\n".len()..];
        assert!(after_header.starts_with("- [[new|"));
    }

    #[test]
    fn test_insert_creates_header() {
        let mut content = "---\ntype: index\n---\n\n# seCall Index\n\n".to_string();
        insert_into_content(
            &mut content,
            "- [[first|first entry]] — 1턴, claude-code, 09:00\n",
        );
        assert!(content.contains("## Sessions\n\n- [[first|"));
    }

    #[test]
    fn test_insert_empty_content() {
        let mut content = String::new();
        insert_into_content(&mut content, "- [[x|x]] — 1턴, a, 00:00\n");
        assert!(content.contains("## Sessions\n\n- [[x|x]]"));
    }

    #[test]
    fn test_insert_preserves_existing() {
        let mut content =
            "## Sessions\n\n- [[a|a]] — 1턴, x, 00:00\n- [[b|b]] — 2턴, y, 01:00\n".to_string();
        insert_into_content(&mut content, "- [[c|c]] — 3턴, z, 02:00\n");
        let idx_c = content.find("[[c|c]]").unwrap();
        let idx_a = content.find("[[a|a]]").unwrap();
        assert!(idx_c < idx_a, "새 엔트리가 기존 엔트리 앞에 삽입");
    }
}
