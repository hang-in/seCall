use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::Serialize;

use crate::store::db::Database;
use crate::store::SessionRepo;
use crate::vault::config::Config;

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LintReport {
    pub findings: Vec<LintFinding>,
    pub summary: LintSummary,
}

#[derive(Debug, Serialize)]
pub struct LintFinding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub session_id: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN",
            Severity::Info => "INFO",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LintSummary {
    pub total_sessions: i64,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub agents: HashMap<String, usize>,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn run_lint(db: &Database, config: &Config) -> Result<LintReport> {
    let mut findings = Vec::new();

    check_vault_files(db, config, &mut findings)?;
    check_orphan_vault_files(db, config, &mut findings)?;
    check_duplicate_sessions(db, &mut findings)?;
    check_missing_embeddings(db, &mut findings)?;
    check_fts_integrity(db, &mut findings)?;
    check_orphan_vectors(db, &mut findings)?;
    check_wiki_frontmatter(config, &mut findings)?;
    check_wiki_source_links(db, config, &mut findings)?;
    check_orphan_sessions(db, config, &mut findings)?;

    let agents = db.agent_counts()?;

    let summary = LintSummary {
        total_sessions: db.count_sessions()?,
        errors: findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Error))
            .count(),
        warnings: findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Warn))
            .count(),
        info: findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Info))
            .count(),
        agents,
    };

    Ok(LintReport { findings, summary })
}

// ─── L001: DB session → vault file exists ────────────────────────────────────

fn check_vault_files(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    for (session_id, vault_path) in db.list_session_vault_paths()? {
        match vault_path {
            None => {
                findings.push(LintFinding {
                    code: "L001".to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "session {}: no vault_path recorded",
                        &session_id[..session_id.len().min(8)]
                    ),
                    session_id: Some(session_id),
                    path: None,
                });
            }
            Some(ref path) => {
                // 상대경로이면 vault root와 join, 절대경로(레거시)이면 그대로 사용
                let check_path = if std::path::Path::new(path).is_absolute() {
                    std::path::PathBuf::from(path)
                } else {
                    config.vault.path.join(path)
                };
                if !check_path.exists() {
                    findings.push(LintFinding {
                        code: "L001".to_string(),
                        severity: Severity::Error,
                        message: format!("vault file missing at {path}"),
                        session_id: Some(session_id),
                        path: Some(path.clone()),
                    });
                }
            }
        }
    }
    Ok(())
}

// ─── L002: vault file → DB session exists ────────────────────────────────────

fn check_orphan_vault_files(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        // 1차: frontmatter에서 session_id 추출 (정확한 UUID)
        let session_id = match extract_session_id_from_file(p) {
            Some(id) => id,
            None => {
                // frontmatter 없으면 파일명 마지막 '_' 이후 prefix로 fallback
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                match stem.rfind('_') {
                    Some(pos) => stem[pos + 1..].to_string(),
                    None => stem.to_string(),
                }
            }
        };

        if session_id.is_empty() {
            continue;
        }

        let exists = if session_id.len() > 8 {
            // Full UUID (frontmatter에서 추출) → 정확한 EXISTS 쿼리
            db.session_exists(&session_id).unwrap_or(true)
        } else {
            // Short prefix (파일명 fallback) → LIKE 쿼리
            db.session_exists_by_prefix(&session_id).unwrap_or(true)
        };

        if !exists {
            findings.push(LintFinding {
                code: "L002".to_string(),
                severity: Severity::Warn,
                message: format!("vault file exists but no DB record: {}", p.display()),
                session_id: Some(session_id),
                path: Some(p.to_string_lossy().to_string()),
            });
        }
    }
    Ok(())
}

// ─── L003: duplicate session IDs ─────────────────────────────────────────────

fn check_duplicate_sessions(db: &Database, findings: &mut Vec<LintFinding>) -> Result<()> {
    // sessions.id is PRIMARY KEY so duplicates are impossible in SQLite,
    // but check via ingest_log for double-ingest attempts
    for (session_id, count) in db.find_duplicate_ingest_entries()? {
        if count > 1 {
            findings.push(LintFinding {
                code: "L003".to_string(),
                severity: Severity::Error,
                message: format!("session ingested {count} times (possible duplicate)"),
                session_id: Some(session_id),
                path: None,
            });
        }
    }
    Ok(())
}

// ─── L004: missing vector embeddings ─────────────────────────────────────────

fn check_missing_embeddings(db: &Database, findings: &mut Vec<LintFinding>) -> Result<()> {
    for session_id in db.find_sessions_without_vectors()? {
        findings.push(LintFinding {
            code: "L004".to_string(),
            severity: Severity::Info,
            message: "no vector embeddings (run `secall embed`)".to_string(),
            session_id: Some(session_id),
            path: None,
        });
    }
    Ok(())
}

// ─── L005: FTS5 integrity ─────────────────────────────────────────────────────

fn check_fts_integrity(db: &Database, findings: &mut Vec<LintFinding>) -> Result<()> {
    let turns_count = db.count_turns()?;
    let fts_count = db.count_fts_rows()?;

    if turns_count != fts_count {
        findings.push(LintFinding {
            code: "L005".to_string(),
            severity: Severity::Error,
            message: format!(
                "FTS5 index has {fts_count} rows but turns table has {turns_count} rows (run `secall reindex`)"
            ),
            session_id: None,
            path: None,
        });
    }
    Ok(())
}

// ─── L007: orphan vectors ─────────────────────────────────────────────────────

fn check_orphan_vectors(db: &Database, findings: &mut Vec<LintFinding>) -> Result<()> {
    for (rowid, session_id) in db.find_orphan_vectors()? {
        findings.push(LintFinding {
            code: "L007".to_string(),
            severity: Severity::Warn,
            message: format!(
                "orphan vector row {rowid}: session_id '{session_id}' not in sessions"
            ),
            session_id: Some(session_id),
            path: None,
        });
    }
    Ok(())
}

// ─── L008: wiki page frontmatter missing ────────────────────────────────────

fn check_wiki_frontmatter(config: &Config, findings: &mut Vec<LintFinding>) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(p).unwrap_or_default();

        if !content.starts_with("---\n") {
            findings.push(LintFinding {
                code: "L008".to_string(),
                severity: Severity::Warn,
                message: format!("wiki page missing frontmatter: {}", p.display()),
                session_id: None,
                path: Some(p.to_string_lossy().to_string()),
            });
            continue;
        }

        let fm_end = content[4..].find("\n---").map(|i| i + 4);
        if let Some(end) = fm_end {
            let fm = &content[4..end];
            for required in &["title:", "type:", "sources:"] {
                if !fm.contains(required) {
                    findings.push(LintFinding {
                        code: "L008".to_string(),
                        severity: Severity::Warn,
                        message: format!(
                            "wiki page missing '{}' in frontmatter: {}",
                            required.trim_end_matches(':'),
                            p.display()
                        ),
                        session_id: None,
                        path: Some(p.to_string_lossy().to_string()),
                    });
                }
            }
        }
    }
    Ok(())
}

// ─── L009: wiki → raw link broken ────────────────────────────────────────────

fn check_wiki_source_links(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.extension().map(|e| e == "md").unwrap_or(false) {
            continue;
        }

        let content = std::fs::read_to_string(p).unwrap_or_default();
        for sid in extract_sources(&content) {
            if !db.session_exists(&sid).unwrap_or(true) {
                findings.push(LintFinding {
                    code: "L009".to_string(),
                    severity: Severity::Error,
                    message: format!("wiki references non-existent session '{sid}'"),
                    session_id: Some(sid),
                    path: Some(p.to_string_lossy().to_string()),
                });
            }
        }
    }
    Ok(())
}

// ─── L010: orphan sessions (not referenced in wiki) ──────────────────────────

fn check_orphan_sessions(
    db: &Database,
    config: &Config,
    findings: &mut Vec<LintFinding>,
) -> Result<()> {
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        return Ok(());
    }

    let mut referenced: HashSet<String> = HashSet::new();
    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(p).unwrap_or_default();
            for sid in extract_sources(&content) {
                referenced.insert(sid);
            }
        }
    }

    for sid in db.list_all_session_ids()? {
        if !referenced.contains(&sid) {
            findings.push(LintFinding {
                code: "L010".to_string(),
                severity: Severity::Info,
                message: "session not referenced in any wiki page".to_string(),
                session_id: Some(sid),
                path: None,
            });
        }
    }
    Ok(())
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// vault 마크다운 파일에서 frontmatter의 session_id 필드를 추출
fn extract_session_id_from_file(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    extract_session_id_from_frontmatter(&content)
}

fn extract_session_id_from_frontmatter(content: &str) -> Option<String> {
    if !content.starts_with("---\n") {
        return None;
    }
    let fm_end = content[4..].find("\n---")?;
    let frontmatter = &content[4..4 + fm_end];

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("session_id:") {
            let value = trimmed.strip_prefix("session_id:").unwrap_or("").trim();
            let id = value.trim_matches('"').trim_matches('\'');
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// Parse `sources: ["id1", "id2"]` from a single frontmatter line.
fn extract_sources(content: &str) -> Vec<String> {
    let mut sources = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("sources:") {
            if let Some(arr_start) = trimmed.find('[') {
                if let Some(arr_end) = trimmed.find(']') {
                    let arr = &trimmed[arr_start + 1..arr_end];
                    for item in arr.split(',') {
                        let s = item.trim().trim_matches('"').trim_matches('\'');
                        if !s.is_empty() {
                            sources.push(s.to_string());
                        }
                    }
                }
            }
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Database;

    fn make_config_tmp() -> (Config, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.vault.path = tmp.path().join("vault");
        std::fs::create_dir_all(&config.vault.path).unwrap();
        (config, tmp)
    }

    #[test]
    fn test_lint_empty_db() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_tmp();
        let report = run_lint(&db, &config).unwrap();
        assert_eq!(report.findings.len(), 0);
        assert_eq!(report.summary.total_sessions, 0);
        assert_eq!(report.summary.errors, 0);
    }

    #[test]
    fn test_lint_fts_mismatch() {
        let db = Database::open_memory().unwrap();
        // Insert a turn but NOT an FTS entry → mismatch
        db.conn()
            .execute_batch("INSERT INTO sessions(id, agent, start_time, ingested_at) VALUES('s1','claude-code','2026-01-01','2026-01-01')")
            .unwrap();
        db.conn()
            .execute_batch("INSERT INTO turns(session_id, turn_index, role, content) VALUES('s1', 0, 'user', 'hello')")
            .unwrap();
        // turns_fts has 0 rows, turns has 1 → L005
        let (config, _tmp) = make_config_tmp();
        let report = run_lint(&db, &config).unwrap();
        let l005 = report.findings.iter().find(|f| f.code == "L005");
        assert!(l005.is_some(), "expected L005 finding");
        assert!(matches!(l005.unwrap().severity, Severity::Error));
    }

    #[test]
    fn test_lint_missing_vault_file() {
        let db = Database::open_memory().unwrap();
        db.conn()
            .execute_batch("INSERT INTO sessions(id, agent, start_time, ingested_at, vault_path) VALUES('s1','claude-code','2026-01-01','2026-01-01','/nonexistent/path/s1.md')")
            .unwrap();
        let (config, _tmp) = make_config_tmp();
        let report = run_lint(&db, &config).unwrap();
        let l001 = report.findings.iter().find(|f| f.code == "L001");
        assert!(l001.is_some(), "expected L001 finding");
    }

    #[test]
    fn test_lint_report_json() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_tmp();
        let report = run_lint(&db, &config).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("findings"));
        assert!(json.contains("summary"));
    }

    // ─── Wiki lint tests ──────────────────────────────────────────────────────

    fn make_config_with_wiki() -> (Config, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.vault.path = tmp.path().join("vault");
        std::fs::create_dir_all(config.vault.path.join("wiki")).unwrap();
        (config, tmp)
    }

    #[test]
    fn test_lint_wiki_no_dir() {
        // wiki/ does not exist → L008~L010 skipped (0 wiki findings)
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_tmp(); // no wiki/ dir
        let report = run_lint(&db, &config).unwrap();
        let wiki_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| matches!(f.code.as_str(), "L008" | "L009" | "L010"))
            .collect();
        assert_eq!(wiki_findings.len(), 0);
    }

    #[test]
    fn test_lint_wiki_missing_frontmatter() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_with_wiki();
        // Write a wiki page without frontmatter
        std::fs::write(
            config.vault.path.join("wiki").join("no-fm.md"),
            "# No frontmatter here\n",
        )
        .unwrap();
        let report = run_lint(&db, &config).unwrap();
        let l008 = report.findings.iter().find(|f| f.code == "L008");
        assert!(
            l008.is_some(),
            "expected L008 finding for missing frontmatter"
        );
        assert!(matches!(l008.unwrap().severity, Severity::Warn));
    }

    #[test]
    fn test_lint_wiki_frontmatter_missing_sources() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_with_wiki();
        // Frontmatter present but missing 'sources:' field
        std::fs::write(
            config.vault.path.join("wiki").join("no-sources.md"),
            "---\ntitle: \"Test\"\ntype: topic\n---\n# Test\n",
        )
        .unwrap();
        let report = run_lint(&db, &config).unwrap();
        let l008_sources = report
            .findings
            .iter()
            .find(|f| f.code == "L008" && f.message.contains("sources"));
        assert!(
            l008_sources.is_some(),
            "expected L008 for missing 'sources' field"
        );
    }

    #[test]
    fn test_lint_wiki_frontmatter_missing_type() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_with_wiki();
        // Frontmatter present but missing 'type:' field
        std::fs::write(
            config.vault.path.join("wiki").join("no-type.md"),
            "---\ntitle: \"Test\"\nsources: []\n---\n# Test\n",
        )
        .unwrap();
        let report = run_lint(&db, &config).unwrap();
        let l008_type = report
            .findings
            .iter()
            .find(|f| f.code == "L008" && f.message.contains("type"));
        assert!(
            l008_type.is_some(),
            "expected L008 for missing 'type' field"
        );
    }

    #[test]
    fn test_lint_wiki_broken_source() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_with_wiki();
        // Write a wiki page referencing a non-existent session
        std::fs::write(
            config.vault.path.join("wiki").join("broken.md"),
            "---\ntitle: \"Test\"\ntype: topic\nsources: [\"nonexistent-session-id\"]\n---\n# Test\n",
        )
        .unwrap();
        let report = run_lint(&db, &config).unwrap();
        let l009 = report.findings.iter().find(|f| f.code == "L009");
        assert!(
            l009.is_some(),
            "expected L009 finding for broken source link"
        );
        assert!(matches!(l009.unwrap().severity, Severity::Error));
    }

    // ─── L002 tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_extract_session_id_from_frontmatter() {
        let content =
            "---\ntype: session\nsession_id: \"abc-123\"\nagent: claude-code\n---\n# Session\n";
        assert_eq!(
            extract_session_id_from_frontmatter(content),
            Some("abc-123".to_string())
        );

        // No frontmatter
        assert_eq!(
            extract_session_id_from_frontmatter("# No frontmatter"),
            None
        );

        // Missing session_id field
        let no_id = "---\ntype: session\nagent: claude-code\n---\n";
        assert_eq!(extract_session_id_from_frontmatter(no_id), None);
    }

    #[test]
    fn test_lint_l002_no_false_positive() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_tmp();

        // DB에 세션 삽입
        db.conn()
            .execute_batch(
                "INSERT INTO sessions(id, agent, start_time, ingested_at) \
                 VALUES('a1b2c3d4-e5f6-7890-abcd-ef1234567890','claude-code','2026-01-01','2026-01-01')",
            )
            .unwrap();

        // vault 파일 생성 (frontmatter에 session_id 포함)
        let sessions_dir = config
            .vault
            .path
            .join("raw")
            .join("sessions")
            .join("2026-01-01");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        std::fs::write(
            sessions_dir.join("claude-code_seCall_a1b2c3d4.md"),
            "---\ntype: session\nsession_id: \"a1b2c3d4-e5f6-7890-abcd-ef1234567890\"\nagent: claude-code\n---\n# Session\n",
        )
        .unwrap();

        let report = run_lint(&db, &config).unwrap();
        let l002: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "L002")
            .collect();
        assert!(
            l002.is_empty(),
            "L002 false positive: 기존 세션을 orphan으로 오탐하면 안 됨"
        );
    }

    #[test]
    fn test_lint_l002_detects_real_orphan() {
        let db = Database::open_memory().unwrap();
        let (config, _tmp) = make_config_tmp();

        // DB에 세션 없음 — vault 파일만 존재
        let sessions_dir = config
            .vault
            .path
            .join("raw")
            .join("sessions")
            .join("2026-01-01");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        std::fs::write(
            sessions_dir.join("claude-code_unknown_deadbeef.md"),
            "---\ntype: session\nsession_id: \"deadbeef-0000-0000-0000-000000000000\"\n---\n",
        )
        .unwrap();

        let report = run_lint(&db, &config).unwrap();
        let l002: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.code == "L002")
            .collect();
        assert_eq!(l002.len(), 1, "L002 should detect real orphan vault file");
    }

    #[test]
    fn test_lint_wiki_orphan_session() {
        let db = Database::open_memory().unwrap();
        // Insert a session that is not referenced in any wiki page
        db.conn()
            .execute_batch(
                "INSERT INTO sessions(id, agent, start_time, ingested_at) \
                 VALUES('orphan-session-1','claude-code','2026-01-01','2026-01-01')",
            )
            .unwrap();
        let (config, _tmp) = make_config_with_wiki();
        // wiki/ exists but has no pages referencing orphan-session-1
        let report = run_lint(&db, &config).unwrap();
        let l010 = report.findings.iter().find(|f| f.code == "L010");
        assert!(l010.is_some(), "expected L010 finding for orphan session");
        assert!(matches!(l010.unwrap().severity, Severity::Info));
    }
}
