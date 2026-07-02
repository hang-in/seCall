use std::path::{Path, PathBuf};

use anyhow::Result;

pub mod config;
pub mod git;
pub mod index;
pub mod init;
pub mod log;

pub use config::Config;
pub use init::init_vault;

use crate::ingest::{
    markdown::{render_session, session_vault_path},
    Session,
};

/// Canonical session storage subdirectory under a vault root.
///
/// P49/v0.5.0 부터 세션 md 는 `raw/.sessions/` (dot-prefix) 에 쓴다. 활성 코드는
/// 직접 `join("raw").join("sessions")` (무점, legacy) 를 조립하지 말고 이 helper 를
/// 사용한다. legacy 무점 경로는 더 이상 디스크에 존재하지 않는다.
pub fn sessions_subdir(vault_path: &Path) -> PathBuf {
    vault_path.join("raw").join(".sessions")
}

/// DB 에 저장된 `vault_path`(구분자 `\`·`/` 혼재 + legacy 무점 `raw/sessions` 가능)를
/// 실제 존재하는 세션 md 파일로 해석한다.
///
/// 해석 우선순위:
/// 1. 저장 경로 그대로 (구분자 정규화)
/// 2. legacy `raw/sessions` → canonical `raw/.sessions` 치환
/// 3. `raw/.sessions` 하위에서 8자리 session_id prefix 로 파일명 탐색
///
/// prefix 다중 매치는 임의 선택하지 않고 에러를 반환한다. 어디에서도 못 찾으면
/// `SessionNotFound`.
pub fn resolve_session_file(
    vault_path: &Path,
    stored_rel: &str,
    session_id: &str,
) -> crate::error::Result<PathBuf> {
    // `\` 와 `/` 를 모두 컴포넌트 구분자로 취급해 OS-native path 로 재조립.
    fn join_normalized(root: &Path, rel: &str) -> PathBuf {
        let mut p = root.to_path_buf();
        for comp in rel.split(['/', '\\']).filter(|c| !c.is_empty()) {
            p.push(comp);
        }
        p
    }

    // (1) 저장 경로 그대로
    let direct = join_normalized(vault_path, stored_rel);
    if direct.is_file() {
        return Ok(direct);
    }

    // (2) legacy 무점 `sessions` 컴포넌트를 `.sessions` 로 치환
    let comps: Vec<&str> = stored_rel
        .split(['/', '\\'])
        .filter(|c| !c.is_empty())
        .collect();
    if comps.contains(&"sessions") {
        let swapped: Vec<&str> = comps
            .iter()
            .map(|c| if *c == "sessions" { ".sessions" } else { *c })
            .collect();
        let cand = join_normalized(vault_path, &swapped.join("/"));
        if cand.is_file() {
            return Ok(cand);
        }
    }

    // (3) prefix 탐색
    let short = &session_id[..session_id.len().min(8)];
    let sessions_dir = sessions_subdir(vault_path);
    let mut matches: Vec<PathBuf> = Vec::new();
    if sessions_dir.exists() {
        for entry in walkdir::WalkDir::new(&sessions_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().map(|x| x == "md").unwrap_or(false) {
                let fname = p.file_name().unwrap_or_default().to_string_lossy();
                if fname.contains(short) {
                    matches.push(p.to_path_buf());
                }
            }
        }
    }
    match matches.len() {
        1 => Ok(matches.into_iter().next().unwrap()),
        0 => Err(crate::SecallError::SessionNotFound(session_id.to_string())),
        n => Err(crate::SecallError::Other(anyhow::anyhow!(
            "ambiguous vault file for session {session_id}: {n} candidates match prefix '{short}'"
        ))),
    }
}

pub struct Vault {
    path: PathBuf,
}

impl Vault {
    pub fn new(path: PathBuf) -> Self {
        Vault { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Canonical `<vault>/raw/.sessions` directory.
    pub fn sessions_dir(&self) -> PathBuf {
        sessions_subdir(&self.path)
    }

    pub fn init(&self) -> Result<()> {
        init_vault(&self.path)
    }

    /// Write session markdown to vault and update index/log
    /// Returns the relative path of the written file (relative to vault root)
    pub fn write_session(&self, session: &Session, tz: chrono_tz::Tz) -> Result<PathBuf> {
        // Render markdown
        let md_content = render_session(session, tz);

        // Determine target path
        let rel_path = session_vault_path(session, tz);
        let abs_path = self.path.join(&rel_path);

        // Create parent directory
        if let Some(parent) = abs_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Atomic write: write to temp then rename
        let tmp_path = abs_path.with_extension("md.tmp");
        std::fs::write(&tmp_path, &md_content)?;
        std::fs::rename(&tmp_path, &abs_path)?;

        // Update index and log
        index::update_index(&self.path, session, &rel_path, tz)?;
        log::append_log(&self.path, session, &rel_path, tz)?;

        Ok(rel_path)
    }

    /// 기존 vault session markdown 의 frontmatter `archived` / `archived_at` 만 in-place 갱신.
    /// 본문은 보존. 파일이 존재하지 않으면 에러.
    pub fn update_session_archive_frontmatter(
        &self,
        vault_rel_path: &str,
        archived: bool,
        archived_at: Option<chrono::DateTime<chrono::Utc>>,
        tz: chrono_tz::Tz,
    ) -> Result<()> {
        let abs = self.path.join(vault_rel_path);
        let content = std::fs::read_to_string(&abs)?;

        let (fm_block, body) = split_frontmatter(&content)?;
        let new_fm = upsert_archive_lines(&fm_block, archived, archived_at, tz);

        let new_content = format!("---\n{new_fm}---\n{body}");
        let tmp = abs.with_extension("md.tmp");
        std::fs::write(&tmp, &new_content)?;
        std::fs::rename(&tmp, &abs)?;
        Ok(())
    }

    /// Check if a session has already been ingested (by ID)
    pub fn session_exists(&self, session_id: &str) -> bool {
        // Walk raw/.sessions/ looking for a file containing the session ID
        let sessions_dir = self.sessions_dir();
        if !sessions_dir.exists() {
            return false;
        }
        for entry in walkdir::WalkDir::new(&sessions_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension().map(|e| e == "md").unwrap_or(false) {
                // Check if filename contains the session ID prefix
                let fname = p.file_name().unwrap_or_default().to_string_lossy();
                // Session ID is embedded as prefix in filename, or in frontmatter
                if fname.contains(&session_id[..session_id.len().min(8)]) {
                    return true;
                }
            }
        }
        false
    }
}

fn split_frontmatter(content: &str) -> Result<(String, String)> {
    // CRLF (Windows) 와 LF 모두 지원하기 위해 우선 LF 로 normalize.
    let normalized = content.replace("\r\n", "\n");
    let stripped = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| anyhow::anyhow!("session markdown missing frontmatter prefix"))?;
    let (fm, body) = stripped
        .split_once("\n---\n")
        .ok_or_else(|| anyhow::anyhow!("session markdown frontmatter not terminated"))?;
    Ok((format!("{fm}\n"), body.to_string()))
}

fn upsert_archive_lines(
    fm: &str,
    archived: bool,
    archived_at: Option<chrono::DateTime<chrono::Utc>>,
    tz: chrono_tz::Tz,
) -> String {
    let mut kept: Vec<String> = fm
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("archived:") && !t.starts_with("archived_at:")
        })
        .map(|l| l.to_string())
        .collect();

    if archived {
        kept.push("archived: true".to_string());
        if let Some(at) = archived_at {
            kept.push(format!(
                "archived_at: \"{}\"",
                at.with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S%:z")
            ));
        }
    }

    kept.iter().map(|l| format!("{l}\n")).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn make_session() -> Session {
        Session {
            id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(),
            agent: AgentKind::ClaudeCode,
            model: Some("claude-opus-4-6".to_string()),
            project: Some("seCall".to_string()),
            cwd: Some(PathBuf::from("/Users/user/seCall")),
            git_branch: Some("main".to_string()),
            host: None,
            start_time: chrono::Utc.with_ymd_and_hms(2026, 4, 5, 5, 30, 0).unwrap(),
            end_time: None,
            turns: vec![Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: "Test session content".to_string(),
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            }],
            total_tokens: TokenUsage {
                input: 100,
                output: 50,
                cached: 0,
            },
            session_type: "interactive".to_string(),
            archived: false,
            archived_at: None,
        }
    }

    #[test]
    fn test_init_vault_creates_dirs() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        assert!(dir.path().join("raw/.sessions").exists());
        assert!(dir.path().join("wiki/projects").exists());
        assert!(dir.path().join("wiki/topics").exists());
        assert!(dir.path().join("wiki/decisions").exists());
    }

    #[test]
    fn test_init_vault_creates_files() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        assert!(dir.path().join("SCHEMA.md").exists());
        assert!(dir.path().join("index.md").exists());
        assert!(dir.path().join("log.md").exists());
    }

    #[test]
    fn test_init_vault_does_not_overwrite() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        // Write custom content
        std::fs::write(dir.path().join("index.md"), "custom content").unwrap();
        // Re-init
        init_vault(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join("index.md")).unwrap();
        assert_eq!(content, "custom content");
    }

    #[test]
    fn test_init_vault_creates_wiki_dirs() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        assert!(dir.path().join("wiki").exists());
        assert!(dir.path().join("wiki/projects").exists());
        assert!(dir.path().join("wiki/topics").exists());
        assert!(dir.path().join("wiki/decisions").exists());
    }

    #[test]
    fn test_init_vault_creates_schema() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        let schema_path = dir.path().join("SCHEMA.md");
        assert!(schema_path.exists());
        let content = std::fs::read_to_string(&schema_path).unwrap();
        assert!(
            content.contains("title:"),
            "SCHEMA.md should document 'title' frontmatter field"
        );
        assert!(
            content.contains("sources:"),
            "SCHEMA.md should document 'sources' frontmatter field"
        );
        assert!(
            content.contains("wiki/projects/"),
            "SCHEMA.md should describe directory rules"
        );
    }

    #[test]
    fn test_init_vault_creates_overview() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        assert!(dir.path().join("wiki/overview.md").exists());
    }

    #[test]
    fn test_init_vault_idempotent_wiki() {
        let dir = TempDir::new().unwrap();
        init_vault(dir.path()).unwrap();
        // Write custom content to wiki/overview.md
        std::fs::write(dir.path().join("wiki/overview.md"), "custom wiki content").unwrap();
        // Re-init should NOT overwrite
        init_vault(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join("wiki/overview.md")).unwrap();
        assert_eq!(content, "custom wiki content");
    }

    #[test]
    fn test_write_session_creates_file() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        let rel_path = vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();

        // 반환값이 상대경로인지 확인
        assert!(rel_path.is_relative());
        assert!(rel_path.starts_with("raw/.sessions/"));

        // 절대경로로 합성 시 파일 존재 및 내용 확인
        let abs_path = dir.path().join(&rel_path);
        assert!(abs_path.exists());
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert!(content.contains("type: session"));
    }

    #[test]
    fn test_write_session_updates_index() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();
        let index = std::fs::read_to_string(dir.path().join("index.md")).unwrap();
        assert!(index.contains("claude-code_seCall_a1b2c3d"));
    }

    #[test]
    fn test_write_session_appends_log() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();
        let log = std::fs::read_to_string(dir.path().join("log.md")).unwrap();
        assert!(log.contains("ingest | claude-code seCall"));
    }

    #[test]
    fn test_session_exists_detects_duplicate() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        assert!(!vault.session_exists(&session.id));
        vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();
        assert!(vault.session_exists(&session.id));
    }

    #[test]
    fn test_config_load_or_default() {
        // No config file → returns default without panic.
        // SECALL_CONFIG_PATH 를 set/remove 하는 동안 vault::config::tests 의
        // ENV_MUTEX 와 race 가 발생하면 다른 테스트가 엉뚱한 path 를 보게 된다.
        // 같은 mutex 를 잡아 직렬화한다.
        let _guard = super::config::ENV_MUTEX
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::set_var("SECALL_CONFIG_PATH", "/nonexistent/path/config.toml");
        let config = Config::load_or_default();
        assert!(config.ingest.tool_output_max_chars > 0);
        std::env::remove_var("SECALL_CONFIG_PATH");
    }

    #[test]
    fn test_update_archive_frontmatter_adds_lines() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        let rel = vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();
        let rel_str = rel.to_string_lossy().to_string();

        vault
            .update_session_archive_frontmatter(
                &rel_str,
                true,
                Some(chrono::Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap()),
                chrono_tz::Tz::UTC,
            )
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join(&rel)).unwrap();
        assert!(
            content.contains("\narchived: true\n"),
            "archived: true missing"
        );
        assert!(content.contains("archived_at:"), "archived_at missing");
        // 본문 보존 확인
        assert!(content.contains("Test session content"));
    }

    #[test]
    fn test_update_archive_frontmatter_removes_lines_on_restore() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let session = make_session();
        let rel = vault.write_session(&session, chrono_tz::Tz::UTC).unwrap();
        let rel_str = rel.to_string_lossy().to_string();

        vault
            .update_session_archive_frontmatter(
                &rel_str,
                true,
                Some(chrono::Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap()),
                chrono_tz::Tz::UTC,
            )
            .unwrap();
        vault
            .update_session_archive_frontmatter(&rel_str, false, None, chrono_tz::Tz::UTC)
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join(&rel)).unwrap();
        assert!(
            !content.contains("archived:"),
            "archived: should be removed"
        );
        assert!(
            !content.contains("archived_at:"),
            "archived_at: should be removed"
        );
    }

    // ─── sessions_subdir / resolve_session_file ────────────────────────────────

    #[test]
    fn sessions_subdir_is_canonical_dot_path() {
        let dir = TempDir::new().unwrap();
        let sub = sessions_subdir(dir.path());
        assert!(sub.ends_with(std::path::Path::new("raw").join(".sessions")));
    }

    fn write_md(root: &std::path::Path, rel: &str) -> std::path::PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "---\nsession_id: x\n---\n\n## Turn 1 — User\n\nhi\n").unwrap();
        p
    }

    #[test]
    fn resolve_direct_dot_path() {
        let dir = TempDir::new().unwrap();
        let real = write_md(
            dir.path(),
            "raw/.sessions/2026-04-01/claude-code_p_abcd1234.md",
        );
        let got = resolve_session_file(
            dir.path(),
            "raw/.sessions/2026-04-01/claude-code_p_abcd1234.md",
            "abcd1234-0000",
        )
        .unwrap();
        assert_eq!(got, real);
    }

    #[test]
    fn resolve_legacy_backslash_no_dot_to_dot_file() {
        // DB 에 저장된 legacy 백슬래시 무점 경로 → 실제 .sessions 파일로 해석
        let dir = TempDir::new().unwrap();
        let real = write_md(
            dir.path(),
            "raw/.sessions/2026-04-01/claude-code_p_abcd1234.md",
        );
        let stored = "raw\\sessions\\2026-04-01\\claude-code_p_abcd1234.md";
        let got = resolve_session_file(dir.path(), stored, "abcd1234-0000").unwrap();
        assert_eq!(got, real);
    }

    #[test]
    fn resolve_by_prefix_when_path_differs() {
        // 저장 경로의 날짜 버킷이 달라도 8자리 prefix 로 탐색
        let dir = TempDir::new().unwrap();
        let real = write_md(
            dir.path(),
            "raw/.sessions/2026-05-09/claude-code_p_deadbeef.md",
        );
        let stored = "raw\\sessions\\2026-04-01\\claude-code_p_deadbeef.md";
        let got = resolve_session_file(dir.path(), stored, "deadbeef-1111").unwrap();
        assert_eq!(got, real);
    }

    #[test]
    fn resolve_missing_file_is_not_found() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(sessions_subdir(dir.path())).unwrap();
        let err = resolve_session_file(
            dir.path(),
            "raw\\sessions\\2026-04-01\\claude-code_p_00000000.md",
            "00000000-2222",
        );
        assert!(matches!(err, Err(crate::SecallError::SessionNotFound(_))));
    }

    #[test]
    fn resolve_ambiguous_prefix_errors() {
        // 동일 8자리 prefix 파일이 둘 → 임의 선택 금지, 에러
        let dir = TempDir::new().unwrap();
        write_md(
            dir.path(),
            "raw/.sessions/2026-04-01/claude-code_p_abcd1234.md",
        );
        write_md(dir.path(), "raw/.sessions/2026-04-02/codex_q_abcd1234.md");
        let stored = "raw\\sessions\\2026-01-01\\missing_abcd1234.md";
        let err = resolve_session_file(dir.path(), stored, "abcd1234-3333");
        assert!(matches!(err, Err(crate::SecallError::Other(_))));
    }
}

#[cfg(test)]
pub mod integration {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn test_full_vault_workflow() {
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();

        let sessions: Vec<Session> = (0..3)
            .map(|i| Session {
                id: format!("session-{:08}", i),
                agent: AgentKind::ClaudeCode,
                model: None,
                project: Some("testproject".to_string()),
                cwd: None,
                git_branch: None,
                host: None,
                start_time: chrono::Utc
                    .with_ymd_and_hms(2026, 4, 5 + i, 0, 0, 0)
                    .unwrap(),
                end_time: None,
                turns: vec![Turn {
                    index: 0,
                    role: Role::User,
                    timestamp: None,
                    content: format!("Session {} content", i),
                    actions: Vec::new(),
                    tokens: None,
                    thinking: None,
                    is_sidechain: false,
                }],
                total_tokens: TokenUsage::default(),
                session_type: "interactive".to_string(),
                archived: false,
                archived_at: None,
            })
            .collect();

        for session in &sessions {
            vault.write_session(session, chrono_tz::Tz::UTC).unwrap();
        }

        let index = std::fs::read_to_string(dir.path().join("index.md")).unwrap();
        assert!(index.contains("Sessions"));

        let log = std::fs::read_to_string(dir.path().join("log.md")).unwrap();
        assert_eq!(log.matches("ingest | claude-code testproject").count(), 3);
    }

    // ─── split_frontmatter cross-platform line ending 회귀 테스트 ──────────

    #[test]
    fn test_split_frontmatter_handles_lf() {
        let content = "---\nfoo: bar\nbaz: qux\n---\nbody content\nmore body\n";
        let (fm, body) = split_frontmatter(content).expect("LF should parse");
        assert!(fm.contains("foo: bar"));
        assert!(fm.contains("baz: qux"));
        assert_eq!(body, "body content\nmore body\n");
    }

    #[test]
    fn test_split_frontmatter_handles_crlf() {
        // Windows 환경에서 작성된 파일은 CRLF 라인 엔딩을 사용.
        let content = "---\r\nfoo: bar\r\nbaz: qux\r\n---\r\nbody content\r\nmore body\r\n";
        let (fm, body) = split_frontmatter(content).expect("CRLF should parse after normalize");
        assert!(fm.contains("foo: bar"));
        assert!(fm.contains("baz: qux"));
        assert_eq!(body, "body content\nmore body\n");
    }

    #[test]
    fn test_split_frontmatter_rejects_missing_prefix() {
        let content = "no frontmatter here";
        let result = split_frontmatter(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_split_frontmatter_rejects_unterminated() {
        let content = "---\nfoo: bar\nbody without terminator\n";
        let result = split_frontmatter(content);
        assert!(result.is_err());
    }
}
