use std::path::Path;

use anyhow::Result;

/// frontmatter 파싱/교정 후 반환되는 구조체
#[derive(Debug, Clone)]
pub struct WikiFrontmatter {
    pub page_type: String,
    pub status: String,
    pub updated_at: String,
    pub sources: Vec<String>,
}

/// Haiku 출력에서 frontmatter를 파싱하고 누락 필드를 보정한 마크다운을 반환
pub fn validate_frontmatter(content: &str, session_ids: &[String]) -> String {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (existing_fm, body) = split_frontmatter(content);

    let mut fm = if let Some(raw) = existing_fm {
        parse_frontmatter_fields(&raw)
    } else {
        WikiFrontmatter {
            page_type: "topic".to_string(),
            status: "draft".to_string(),
            updated_at: today.clone(),
            sources: vec![],
        }
    };

    // 필드 보정
    if fm.page_type.is_empty() {
        fm.page_type = "topic".to_string();
    }
    if fm.status.is_empty() {
        fm.status = "draft".to_string();
    }
    if fm.updated_at.is_empty() {
        fm.updated_at = today;
    }

    // sources에 세션 ID 추가 (중복 제거)
    for sid in session_ids {
        if !fm.sources.contains(sid) {
            fm.sources.push(sid.clone());
        }
    }

    format_with_frontmatter(&fm, &body)
}

/// 기존 위키 페이지와 병합. 기존 페이지가 없으면 new_content 그대로 반환.
pub fn merge_with_existing(
    wiki_dir: &Path,
    page_path: &str,
    new_content: &str,
    session_ids: &[String],
) -> Result<String> {
    let full_path = wiki_dir.join(page_path);
    if !full_path.exists() {
        return Ok(new_content.to_string());
    }

    let existing = std::fs::read_to_string(&full_path)?;
    let (existing_fm_raw, _existing_body) = split_frontmatter(&existing);
    let (new_fm_raw, new_body) = split_frontmatter(new_content);

    // 기존 sources 파싱
    let mut merged_fm = if let Some(raw) = existing_fm_raw {
        parse_frontmatter_fields(&raw)
    } else {
        WikiFrontmatter {
            page_type: "topic".to_string(),
            status: "draft".to_string(),
            updated_at: String::new(),
            sources: vec![],
        }
    };

    // 이미 세션이 포함되어 있으면 skip (sources 추가 전에 체크)
    let all_already_present = !session_ids.is_empty()
        && session_ids
            .iter()
            .all(|sid| merged_fm.sources.contains(sid));
    if all_already_present {
        return Ok(existing.to_string());
    }

    // 새 frontmatter에서 sources 가져오기
    if let Some(raw) = new_fm_raw {
        let new_fm = parse_frontmatter_fields(&raw);
        for s in new_fm.sources {
            if !merged_fm.sources.contains(&s) {
                merged_fm.sources.push(s);
            }
        }
    }
    for sid in session_ids {
        if !merged_fm.sources.contains(sid) {
            merged_fm.sources.push(sid.clone());
        }
    }

    merged_fm.updated_at = chrono::Utc::now().format("%Y-%m-%d").to_string();

    Ok(format_with_frontmatter(&merged_fm, new_body.trim()))
}

/// 본문에서 세션 ID 패턴을 Obsidian 링크로 변환
/// vault_paths: session_id → vault 상대경로 (예: "raw/sessions/2026-04-13/claude-code_proj_86b9d1fa.md")
/// vault 경로가 없는 세션은 세션 ID 자체를 링크 타깃으로 사용
/// wiki_pages: wiki/ 하위 페이지 경로 목록 (확장자 제거, 예: ["projects/seCall", "topics/rust"])
pub fn insert_obsidian_links(
    content: &str,
    session_ids: &[String],
    vault_paths: &std::collections::HashMap<String, String>,
    wiki_pages: &[String],
) -> String {
    let mut result = content.to_string();

    for sid in session_ids {
        let short = &sid[..8.min(sid.len())];

        // 링크 타깃: vault 경로에서 .md 확장자를 제거 (Obsidian 규약)
        let link_target = if let Some(vp) = vault_paths.get(sid) {
            vp.trim_end_matches(".md").to_string()
        } else {
            sid.clone()
        };
        let link = format!("[[{}|{}]]", link_target, short);

        // 모든 매치를 치환하되, 이미 [[ ]] 안에 있는 매치는 제외
        let mut new_result = String::new();
        let mut remaining = result.as_str();

        while let Some(pos) = remaining.find(short) {
            let before = &remaining[..pos];
            let abs_pos = result.len() - remaining.len() + pos;

            if is_in_obsidian_link(&result, abs_pos) {
                new_result.push_str(&remaining[..pos + short.len()]);
            } else {
                new_result.push_str(before);
                new_result.push_str(&link);
            }
            remaining = &remaining[pos + short.len()..];
        }
        new_result.push_str(remaining);
        result = new_result;
    }

    // 위키 페이지 간 내부 링크 생성
    for page_path in wiki_pages {
        // 페이지 제목 = 경로의 마지막 파일명 (확장자 없이)
        let title = page_path.rsplit('/').next().unwrap_or(page_path);

        // 3자 미만 제목은 오탐 위험 → skip
        if title.len() < 3 {
            continue;
        }

        let link = format!("[[{}|{}]]", page_path, title);
        let mut new_result = String::new();
        let mut remaining = result.as_str();

        while let Some(pos) = remaining.find(title) {
            let abs_pos = result.len() - remaining.len() + pos;

            if is_in_obsidian_link(&result, abs_pos) {
                new_result.push_str(&remaining[..pos + title.len()]);
            } else {
                new_result.push_str(&remaining[..pos]);
                new_result.push_str(&link);
            }
            remaining = &remaining[pos + title.len()..];
        }
        new_result.push_str(remaining);
        result = new_result;
    }

    result
}

/// 절대 위치 `abs_pos`의 텍스트가 이미 `[[ ]]` 링크 안에 있는지 확인
fn is_in_obsidian_link(full_str: &str, abs_pos: usize) -> bool {
    let prefix = &full_str[..abs_pos];
    let suffix = &full_str[abs_pos..];
    if let Some(bracket_start) = prefix.rfind("[[") {
        let between = &prefix[bracket_start + 2..];
        let still_open = !between.contains("]]");
        still_open && suffix.contains("]]")
    } else {
        false
    }
}

/// markdownlint-cli2가 설치되어 있으면 실행
pub fn run_markdownlint(file_path: &Path) -> Result<Option<String>> {
    // which markdownlint-cli2
    let which = std::process::Command::new("which")
        .arg("markdownlint-cli2")
        .output();

    match which {
        Ok(output) if output.status.success() => {
            let result = std::process::Command::new("markdownlint-cli2")
                .arg("--fix")
                .arg(file_path)
                .output()?;

            if result.status.success() {
                Ok(Some("markdownlint --fix applied".to_string()))
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                Ok(Some(format!("markdownlint warnings: {}", stderr.trim())))
            }
        }
        _ => {
            tracing::debug!("markdownlint-cli2 not found, skipping lint");
            Ok(None)
        }
    }
}

// ─── 내부 유틸 ──────────────────────────────────────────────────────────

/// "---\n...\n---\n" frontmatter와 본문을 분리
fn split_frontmatter(content: &str) -> (Option<String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // 첫 "---" 이후 두 번째 "---" 찾기
    if let Some(end_pos) = trimmed[3..].find("\n---") {
        let fm = trimmed[3..3 + end_pos].trim().to_string();
        let body = trimmed[3 + end_pos + 4..].to_string();
        (Some(fm), body)
    } else {
        (None, content.to_string())
    }
}

/// frontmatter 필드를 간이 파싱 (YAML 파서 없이)
fn parse_frontmatter_fields(raw: &str) -> WikiFrontmatter {
    let mut page_type = String::new();
    let mut status = String::new();
    let mut updated_at = String::new();
    let mut sources: Vec<String> = vec![];

    for line in raw.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("type:") {
            page_type = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("status:") {
            status = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("updated_at:") {
            updated_at = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("updated:") {
            // Haiku가 updated 사용할 수 있음
            if updated_at.is_empty() {
                updated_at = val.trim().to_string();
            }
        } else if let Some(val) = line.strip_prefix("date:") {
            // Haiku가 date 사용할 수 있음
            if updated_at.is_empty() {
                updated_at = val.trim().to_string();
            }
        } else if let Some(val) = line.strip_prefix("sources:") {
            // [id1, id2] 형식 파싱
            let val = val.trim();
            if val.starts_with('[') {
                sources = val
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        } else if line.starts_with("- ") && !sources.is_empty() {
            // YAML 리스트 형식의 sources 항목
        } else if let Some(val) = line.strip_prefix("title:") {
            // Haiku가 title 대신 type을 안 넣었을 때 — title은 무시
            let _ = val;
        }
    }

    // sources가 YAML 리스트 형식일 때
    if sources.is_empty() {
        let mut in_sources = false;
        for line in raw.lines() {
            let line = line.trim();
            if line.starts_with("sources:") {
                in_sources = true;
                continue;
            }
            if in_sources {
                if let Some(val) = line.strip_prefix("- ") {
                    sources.push(val.trim().trim_matches('"').trim_matches('\'').to_string());
                } else if !line.is_empty() {
                    in_sources = false;
                }
            }
        }
    }

    WikiFrontmatter {
        page_type,
        status,
        updated_at,
        sources,
    }
}

/// WikiFrontmatter + body를 마크다운으로 조합
fn format_with_frontmatter(fm: &WikiFrontmatter, body: &str) -> String {
    let sources_str = if fm.sources.is_empty() {
        "[]".to_string()
    } else {
        format!(
            "[{}]",
            fm.sources
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    format!(
        "---\ntype: {}\nstatus: {}\nupdated_at: {}\nsources: {}\n---\n{}",
        fm.page_type, fm.status, fm.updated_at, sources_str, body
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_no_frontmatter() {
        let content = "# My Page\n\nSome content";
        let result = validate_frontmatter(content, &["sess-001".to_string()]);
        assert!(result.starts_with("---\n"));
        assert!(result.contains("type: topic"));
        assert!(result.contains("status: draft"));
        assert!(result.contains("sess-001"));
        assert!(result.contains("# My Page"));
    }

    #[test]
    fn test_validate_existing_frontmatter() {
        let content = "---\ntype: project\nstatus: done\nupdated_at: 2026-01-01\nsources: [\"old-id\"]\n---\n# Page";
        let result = validate_frontmatter(content, &["new-id".to_string()]);
        assert!(result.contains("type: project"));
        assert!(result.contains("status: done"));
        assert!(result.contains("old-id"));
        assert!(result.contains("new-id"));
    }

    #[test]
    fn test_validate_sources_dedup() {
        let content = "---\ntype: topic\nsources: [\"abc\"]\n---\nBody";
        let result = validate_frontmatter(content, &["abc".to_string()]);
        // abc가 한번만 나와야 함
        let count = result.matches("abc").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_validate_haiku_date_mapping() {
        let content = "---\ntype: topic\ndate: 2026-04-13\n---\nBody";
        let result = validate_frontmatter(content, &[]);
        assert!(result.contains("updated_at: 2026-04-13"));
    }

    #[test]
    fn test_merge_no_existing() {
        let dir = tempfile::tempdir().unwrap();
        let result =
            merge_with_existing(dir.path(), "topics/test.md", "# New\n\nContent", &[]).unwrap();
        assert_eq!(result, "# New\n\nContent");
    }

    #[test]
    fn test_merge_replaces_body_keeps_sources_union() {
        let dir = tempfile::tempdir().unwrap();
        let topics_dir = dir.path().join("topics");
        std::fs::create_dir_all(&topics_dir).unwrap();
        std::fs::write(
            topics_dir.join("test.md"),
            "---\ntype: topic\nstatus: draft\nupdated_at: 2026-01-01\nsources: [\"old\"]\n---\n# Existing\n\nOld content",
        )
        .unwrap();

        let new_content = "---\ntype: topic\nsources: [\"new\"]\n---\n# New section\n\nNew content";
        let result = merge_with_existing(
            dir.path(),
            "topics/test.md",
            new_content,
            &["new".to_string()],
        )
        .unwrap();

        assert!(!result.contains("Old content"));
        assert!(result.contains("New content"));
        assert!(result.contains("\"old\""));
        assert!(result.contains("\"new\""));
    }

    #[test]
    fn test_merge_skip_duplicate_session() {
        let dir = tempfile::tempdir().unwrap();
        let topics_dir = dir.path().join("topics");
        std::fs::create_dir_all(&topics_dir).unwrap();
        let existing = "---\ntype: topic\nsources: [\"sess-001\"]\n---\n# Page\n\nContent";
        std::fs::write(topics_dir.join("test.md"), existing).unwrap();

        let result = merge_with_existing(
            dir.path(),
            "topics/test.md",
            "---\nsources: []\n---\nNew",
            &["sess-001".to_string()],
        )
        .unwrap();

        // 이미 포함된 세션이므로 기존 내용 그대로
        assert_eq!(result, existing);
    }

    #[test]
    fn test_merge_creates_when_existing_absent() {
        let dir = tempfile::tempdir().unwrap();
        let new_content = "---\ntype: topic\nsources:\n  - sess-A\n---\n\n## Body";
        let merged = merge_with_existing(
            dir.path(),
            "topics/new.md",
            new_content,
            &["sess-A".to_string()],
        )
        .unwrap();

        assert_eq!(merged, new_content);
    }

    #[test]
    fn test_merge_preserves_sources_order() {
        let dir = tempfile::tempdir().unwrap();
        let topics_dir = dir.path().join("topics");
        std::fs::create_dir_all(&topics_dir).unwrap();
        std::fs::write(
            topics_dir.join("ordered.md"),
            "---\ntype: topic\nsources:\n  - sess-A\n  - sess-B\n---\n\n## old",
        )
        .unwrap();

        let merged = merge_with_existing(
            dir.path(),
            "topics/ordered.md",
            "---\ntype: topic\nsources:\n  - sess-C\n---\n\n## new",
            &["sess-C".to_string()],
        )
        .unwrap();

        let pos_a = merged.find("sess-A").unwrap();
        let pos_b = merged.find("sess-B").unwrap();
        let pos_c = merged.find("sess-C").unwrap();
        assert!(pos_a < pos_b && pos_b < pos_c);
    }

    #[test]
    fn test_insert_obsidian_links_with_vault_path() {
        let content = "Session 86b9d1fa was interesting";
        let sid = "86b9d1fa-fccf-4e4c-b562-b1258be431e2".to_string();
        let mut vault_paths = std::collections::HashMap::new();
        vault_paths.insert(
            sid.clone(),
            "raw/sessions/2026-04-13/claude-code_seCall_86b9d1fa.md".to_string(),
        );
        let result = insert_obsidian_links(content, &[sid], &vault_paths, &[]);
        assert!(result.contains("[[raw/sessions/2026-04-13/claude-code_seCall_86b9d1fa|86b9d1fa]]"));
    }

    #[test]
    fn test_insert_obsidian_links_no_vault_path_fallback() {
        let content = "Session 86b9d1fa was interesting";
        let sid = "86b9d1fa-fccf-4e4c-b562-b1258be431e2".to_string();
        let vault_paths = std::collections::HashMap::new(); // 빈 맵
        let result = insert_obsidian_links(content, std::slice::from_ref(&sid), &vault_paths, &[]);
        // vault_path 없으면 full UUID를 링크 타깃으로
        assert!(result.contains(&format!("[[{}|86b9d1fa]]", sid)));
    }

    #[test]
    fn test_insert_obsidian_links_skip_existing() {
        let content = "Already linked [[some/path|86b9d1fa]] here";
        let sid = "86b9d1fa-fccf-4e4c-b562-b1258be431e2".to_string();
        let vault_paths = std::collections::HashMap::new();
        let result = insert_obsidian_links(content, &[sid], &vault_paths, &[]);
        // 이미 [[]] 안에 있으므로 변환하지 않음 — 원본 그대로
        assert_eq!(result, content);
    }

    #[test]
    fn test_insert_obsidian_links_multiple_occurrences() {
        let content = "First 86b9d1fa and second 86b9d1fa reference";
        let sid = "86b9d1fa-fccf-4e4c-b562-b1258be431e2".to_string();
        let mut vault_paths = std::collections::HashMap::new();
        vault_paths.insert(
            sid.clone(),
            "raw/sessions/2026-04-13/claude-code_proj_86b9d1fa.md".to_string(),
        );
        let result = insert_obsidian_links(content, &[sid], &vault_paths, &[]);
        // 두 곳 모두 치환
        let link = "[[raw/sessions/2026-04-13/claude-code_proj_86b9d1fa|86b9d1fa]]";
        assert_eq!(result.matches(link).count(), 2);
    }

    #[test]
    fn test_insert_wiki_page_links() {
        let content = "seCall 프로젝트에서 rust를 사용했다.";
        let wiki_pages = vec!["projects/seCall".to_string()];
        let result = insert_obsidian_links(content, &[], &Default::default(), &wiki_pages);
        assert!(result.contains("[[projects/seCall|seCall]]"));
    }

    #[test]
    fn test_insert_wiki_page_links_skip_existing() {
        let content = "이미 링크된 [[projects/seCall|seCall]] 참조";
        let wiki_pages = vec!["projects/seCall".to_string()];
        let result = insert_obsidian_links(content, &[], &Default::default(), &wiki_pages);
        // 이미 [[]] 안에 있으므로 변환 없음 — 원본과 동일
        assert_eq!(result, content);
    }

    #[test]
    fn test_insert_wiki_page_links_short_title_skip() {
        // 2자 이하 제목은 오탐 위험 → skip
        let content = "AI를 사용했다.";
        let wiki_pages = vec!["topics/AI".to_string()];
        let result = insert_obsidian_links(content, &[], &Default::default(), &wiki_pages);
        assert!(!result.contains("[[topics/AI"));
    }

    #[test]
    fn test_split_frontmatter() {
        let (fm, body) = split_frontmatter("---\ntype: topic\n---\n# Body");
        assert_eq!(fm.unwrap(), "type: topic");
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_split_no_frontmatter() {
        let (fm, body) = split_frontmatter("# Just body");
        assert!(fm.is_none());
        assert!(body.contains("# Just body"));
    }
}
