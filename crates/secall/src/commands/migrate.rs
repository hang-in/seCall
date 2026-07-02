use anyhow::Result;
use secall_core::{
    ingest::markdown::{extract_summary_from_body, parse_session_frontmatter},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub fn run_summary(dry_run: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    let sessions_dir = secall_core::vault::sessions_subdir(&config.vault.path);
    if !sessions_dir.exists() {
        println!("No vault sessions directory found.");
        return Ok(());
    }

    let mut updated = 0usize;
    let mut skipped_has_summary = 0usize;
    let mut skipped_no_user_turn = 0usize;
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

        if fm.summary.is_some() {
            skipped_has_summary += 1;
            continue;
        }

        let summary = match extract_summary_from_body(&content) {
            Some(s) => s,
            None => {
                skipped_no_user_turn += 1;
                continue;
            }
        };

        if dry_run {
            println!("[dry-run] {} → \"{}\"", path.display(), summary);
            updated += 1;
            continue;
        }

        // MD 파일 frontmatter에 summary 삽입
        match insert_summary_into_frontmatter(&content, &summary) {
            Ok(new_content) => {
                // atomic write: 임시 파일 후 rename
                let tmp_path = path.with_extension("md.tmp");
                if let Err(e) = std::fs::write(&tmp_path, &new_content) {
                    tracing::warn!(path = %path.display(), error = %e, "failed to write tmp");
                    errors += 1;
                    continue;
                }
                if let Err(e) = std::fs::rename(&tmp_path, path) {
                    tracing::warn!(path = %path.display(), error = %e, "failed to rename");
                    let _ = std::fs::remove_file(&tmp_path);
                    errors += 1;
                    continue;
                }
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "frontmatter insert failed");
                errors += 1;
                continue;
            }
        }

        // DB 업데이트
        if !fm.session_id.is_empty() {
            if let Err(e) = db.update_session_summary(&fm.session_id, &summary) {
                tracing::warn!(session_id = %fm.session_id, error = %e, "DB update failed");
            }
        }

        eprintln!("  updated: {}", path.display());
        updated += 1;
    }

    eprintln!(
        "\nMigrate summary: {} updated, {} skipped (already has summary), {} skipped (no user turn), {} errors",
        updated, skipped_has_summary, skipped_no_user_turn, errors
    );
    Ok(())
}

/// frontmatter에서 `status:` 라인 직전에 `summary: "..."` 삽입.
/// status: 라인이 없으면 `---` 종결자 직전에 삽입.
fn insert_summary_into_frontmatter(content: &str, summary: &str) -> Result<String> {
    let after_open = content
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("no opening --- found"))?;

    let (fm_str, rest) = after_open
        .split_once("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("no closing --- found"))?;

    let escaped = fm_str
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        // summary 자체의 이스케이프는 이미 extract_summary_from_body에서 안된 상태 — 여기서 직접 escape
        ;
    // summary 값 이스케이프 (실제 summary 문자열에 적용)
    let escaped_summary = summary.replace('\\', "\\\\").replace('"', "\\\"");
    let summary_line = format!("summary: \"{escaped_summary}\"\n");

    let new_fm = if let Some(pos) = fm_str.find("\nstatus:") {
        // `\nstatus:` 앞에 삽입 → `\nsummary: "..."\nstatus:`
        let (before, after) = fm_str.split_at(pos);
        format!("{before}\n{summary_line}{}", &after[1..]) // after[1..] to skip the leading \n
    } else {
        // status: 없으면 끝에 추가 — 마지막 필드와 같은 줄이 되지 않도록 개행 보장
        if fm_str.ends_with('\n') {
            format!("{fm_str}{summary_line}")
        } else {
            format!("{fm_str}\n{summary_line}")
        }
    };

    let _ = escaped; // suppress unused warning
    Ok(format!("---\n{new_fm}\n---\n{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_summary_before_status() {
        let content = "---\nagent: claude-code\nstatus: raw\n---\nbody\n";
        let result = insert_summary_into_frontmatter(content, "test summary").unwrap();
        assert!(result.contains("summary: \"test summary\"\nstatus: raw"));
    }

    #[test]
    fn test_insert_summary_escape() {
        let content = "---\nagent: claude-code\nstatus: raw\n---\nbody\n";
        let result = insert_summary_into_frontmatter(content, r#"say "hi""#).unwrap();
        assert!(result.contains(r#"summary: "say \"hi\"""#));
    }

    #[test]
    fn test_insert_summary_no_status_newline() {
        // status: 라인이 없는 frontmatter — 마지막 필드와 summary가 같은 줄에 합쳐지면 안 됨
        let content = "---\nagent: claude-code\nhost: myhost\n---\nbody\n";
        let result = insert_summary_into_frontmatter(content, "hello").unwrap();
        // "host: myhost\nsummary: ..." 형태여야 함
        assert!(
            result.contains("host: myhost\nsummary: \"hello\""),
            "summary must be on its own line: {result}"
        );
    }
}
