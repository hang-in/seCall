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
        let sessions_dir = self.path.join("raw").join(".sessions");
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
