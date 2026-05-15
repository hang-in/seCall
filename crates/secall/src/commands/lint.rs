use anyhow::Result;
use secall_core::{
    ingest::lint::{run_lint, Severity},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub fn run(json: bool, errors_only: bool, fix: bool, fix_orphan_vault: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let report = run_lint(&db, &config)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        if fix {
            run_fix(&db, &report)?;
        }
        if fix_orphan_vault {
            run_fix_orphan_vault(&config, &report)?;
        }
        return Ok(());
    }

    // Text output
    println!("secall lint report");
    println!("==================");

    let mut printed = 0;
    for finding in &report.findings {
        if errors_only && !matches!(finding.severity, Severity::Error) {
            continue;
        }
        let sev = finding.severity.as_str();
        let sid = finding
            .session_id
            .as_deref()
            .map(|s| format!("session {}: ", &s[..s.len().min(8)]))
            .unwrap_or_default();
        println!("{} [{sev:5}] {sid}{}", finding.code, finding.message);
        printed += 1;
    }

    if printed == 0 {
        println!("No issues found.");
    }

    println!();
    println!(
        "Summary: {} sessions, {} errors, {} warnings, {} info",
        report.summary.total_sessions,
        report.summary.errors,
        report.summary.warnings,
        report.summary.info,
    );

    if !report.summary.agents.is_empty() {
        let agent_str: Vec<String> = {
            let mut pairs: Vec<_> = report.summary.agents.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            pairs.iter().map(|(k, v)| format!("{k}({v})")).collect()
        };
        println!("Agents: {}", agent_str.join(", "));
    }

    // --fix: auto-repair L001 (stale DB records)
    if fix {
        run_fix(&db, &report)?;
    }

    // P54 --fix-orphan-vault: L002 (vault md 가 DB 에 없는 경우) 를 archive 로 이동
    if fix_orphan_vault {
        run_fix_orphan_vault(&config, &report)?;
    }

    // Exit with code 1 if there are errors (after fix, re-count)
    let remaining_errors = if fix || fix_orphan_vault {
        // Re-run lint to get updated count
        let updated = run_lint(&db, &config)?;
        updated.summary.errors
    } else {
        report.summary.errors
    };

    if remaining_errors > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn run_fix(db: &Database, report: &secall_core::ingest::lint::LintReport) -> Result<()> {
    let stale: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.code == "L001" && f.session_id.is_some())
        .filter(|f| f.message.contains("vault file missing"))
        .filter_map(|f| f.session_id.as_deref())
        .collect();

    if stale.is_empty() {
        eprintln!("[fix] No stale DB records to clean up.");
        return Ok(());
    }

    eprintln!(
        "[fix] Removing {} stale DB record(s) with missing vault files...",
        stale.len()
    );
    for session_id in &stale {
        match db.delete_session_full(session_id) {
            Ok(()) => eprintln!("  deleted {}", &session_id[..session_id.len().min(8)]),
            Err(e) => eprintln!(
                "  failed to delete {}: {e}",
                &session_id[..session_id.len().min(8)]
            ),
        }
    }
    eprintln!("[fix] Done. {} record(s) removed.", stale.len());
    Ok(())
}

/// P54: L002 finding 의 orphan vault md (DB 에 session 없음) 를 archive 디렉토리로 이동.
///
/// **삭제하지 않고 이동** — 사용자 의도 외 데이터 손실 회피. archive 경로:
/// `<vault.path>/archive/orphan-<YYYY-MM-DD>/<원래 상대경로>`.
/// 다시 ingest 하면 자동으로 raw/.sessions/ 에 복귀.
fn run_fix_orphan_vault(
    config: &Config,
    report: &secall_core::ingest::lint::LintReport,
) -> Result<()> {
    let orphans: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.code == "L002")
        .filter_map(|f| f.path.as_deref())
        .collect();

    if orphans.is_empty() {
        eprintln!("[fix-orphan-vault] No orphan vault files.");
        return Ok(());
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let archive_root = config
        .vault
        .path
        .join("archive")
        .join(format!("orphan-{today}"));

    eprintln!(
        "[fix-orphan-vault] Moving {} orphan vault file(s) → {}",
        orphans.len(),
        archive_root.display()
    );

    let mut moved = 0usize;
    let mut failed = 0usize;
    for src_str in &orphans {
        let src = std::path::PathBuf::from(src_str);
        // relative path 추출 — vault.path 기준
        let rel = match src.strip_prefix(&config.vault.path) {
            Ok(r) => r.to_path_buf(),
            // 절대경로 매칭 실패 시 파일명만 사용 (안전 fallback)
            Err(_) => src
                .file_name()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| src.clone()),
        };
        let dst = archive_root.join(&rel);
        if let Some(parent) = dst.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("  failed to create dir {}: {e}", parent.display());
                failed += 1;
                continue;
            }
        }
        match std::fs::rename(&src, &dst) {
            Ok(()) => {
                eprintln!(
                    "  moved {} → archive/orphan-{today}/{}",
                    src.display(),
                    rel.display()
                );
                moved += 1;
            }
            Err(e) => {
                eprintln!("  failed to move {}: {e}", src.display());
                failed += 1;
            }
        }
    }

    eprintln!("[fix-orphan-vault] Done. {moved} moved, {failed} failed.",);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secall_core::ingest::lint::{LintFinding, LintReport, LintSummary};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn empty_summary() -> LintSummary {
        LintSummary {
            total_sessions: 0,
            errors: 0,
            warnings: 0,
            info: 0,
            agents: HashMap::new(),
        }
    }

    #[test]
    fn test_run_fix_orphan_vault_moves_files_to_archive() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().to_path_buf();
        std::fs::create_dir_all(vault.join("raw/.sessions/2026-05-01")).unwrap();
        let md = vault.join("raw/.sessions/2026-05-01/claude-code_test_abc12345.md");
        std::fs::write(&md, "---\ntype: session\n---\n# test").unwrap();

        let mut config = Config::default();
        config.vault.path = vault.clone();

        let report = LintReport {
            findings: vec![LintFinding {
                code: "L002".to_string(),
                severity: Severity::Warn,
                message: "vault file exists but no DB record".to_string(),
                session_id: Some("abc12345".to_string()),
                path: Some(md.to_string_lossy().to_string()),
            }],
            summary: empty_summary(),
        };

        run_fix_orphan_vault(&config, &report).unwrap();

        // 원본 파일이 사라졌는지
        assert!(!md.exists(), "원본 md 가 이동됐어야 함");

        // archive/orphan-{today}/raw/.sessions/.../ 로 옮겨졌는지
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let moved = vault
            .join("archive")
            .join(format!("orphan-{today}"))
            .join("raw/.sessions/2026-05-01/claude-code_test_abc12345.md");
        assert!(moved.exists(), "archive 안으로 이동: {}", moved.display());
    }

    #[test]
    fn test_run_fix_orphan_vault_no_findings_does_nothing() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.vault.path = tmp.path().to_path_buf();

        let report = LintReport {
            findings: vec![],
            summary: empty_summary(),
        };

        run_fix_orphan_vault(&config, &report).unwrap();

        // L002 finding 없으면 archive 디렉토리도 만들지 않음
        assert!(!tmp.path().join("archive").exists());
    }

    #[test]
    fn test_run_fix_orphan_vault_ignores_non_l002_findings() {
        let tmp = TempDir::new().unwrap();
        let mut config = Config::default();
        config.vault.path = tmp.path().to_path_buf();

        // L001 / L003 만 있으면 무시
        let report = LintReport {
            findings: vec![
                LintFinding {
                    code: "L001".to_string(),
                    severity: Severity::Error,
                    message: "vault file missing".to_string(),
                    session_id: Some("xyz".to_string()),
                    path: None,
                },
                LintFinding {
                    code: "L003".to_string(),
                    severity: Severity::Error,
                    message: "duplicate".to_string(),
                    session_id: Some("dup".to_string()),
                    path: None,
                },
            ],
            summary: empty_summary(),
        };

        run_fix_orphan_vault(&config, &report).unwrap();

        assert!(!tmp.path().join("archive").exists());
    }
}
