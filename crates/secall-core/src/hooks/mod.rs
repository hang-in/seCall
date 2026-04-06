use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::ingest::Session;
use crate::vault::config::Config;

/// Expand `~` prefix to home directory path.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Run the configured post-ingest hook, if any.
///
/// The hook script receives session metadata via environment variables:
/// - `SECALL_SESSION_ID`
/// - `SECALL_AGENT`
/// - `SECALL_PROJECT`
/// - `SECALL_VAULT_PATH`
/// - `SECALL_TURNS`
/// - `SECALL_DATE`
pub fn run_post_ingest_hook(config: &Config, session: &Session, vault_path: &Path) -> Result<()> {
    let hook_path_str = match &config.hooks.post_ingest {
        Some(p) => p.clone(),
        None => return Ok(()), // not configured
    };

    let hook_path = expand_tilde(&hook_path_str);

    if !hook_path.exists() {
        tracing::warn!(path = %hook_path.display(), "post_ingest hook not found");
        return Ok(());
    }

    let timeout_secs = config.hooks.hook_timeout_secs.unwrap_or(30);

    let mut child = std::process::Command::new(&hook_path)
        .env("SECALL_SESSION_ID", &session.id)
        .env("SECALL_AGENT", session.agent.as_str())
        .env("SECALL_PROJECT", session.project.as_deref().unwrap_or(""))
        .env("SECALL_VAULT_PATH", vault_path)
        .env("SECALL_TURNS", session.turns.len().to_string())
        .env(
            "SECALL_DATE",
            session.start_time.format("%Y-%m-%d").to_string(),
        )
        .spawn()?;

    // Wait with timeout using a polling approach
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    tracing::warn!(timeout_secs, "post_ingest hook timed out");
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    };

    if !status.success() {
        tracing::warn!(status = %status, "post_ingest hook exited with non-zero status");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde_home() {
        let result = expand_tilde("~/foo/bar");
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home.join("foo/bar"));
    }

    #[test]
    fn test_expand_tilde_no_tilde() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_expand_tilde_only() {
        let result = expand_tilde("~");
        let home = dirs::home_dir().unwrap();
        assert_eq!(result, home);
    }

    #[test]
    fn test_no_hook_configured_skips() {
        use crate::vault::config::Config;
        let config = Config::default();
        // Default has no post_ingest hook
        let session = make_test_session();
        let result = run_post_ingest_hook(&config, &session, Path::new("/tmp/test.md"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_hook_file_no_error() {
        use crate::vault::config::{Config, HooksConfig};
        let config = Config {
            hooks: HooksConfig {
                post_ingest: Some("/tmp/nonexistent_hook_12345.sh".to_string()),
                hook_timeout_secs: None,
            },
            ..Config::default()
        };
        let session = make_test_session();
        // Should return Ok even if hook file doesn't exist
        let result = run_post_ingest_hook(&config, &session, Path::new("/tmp/test.md"));
        assert!(result.is_ok());
    }

    fn make_test_session() -> Session {
        use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
        use chrono::{TimeZone, Utc};
        Session {
            id: "test-session-id".to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: Some("test".to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc.with_ymd_and_hms(2026, 4, 5, 0, 0, 0).unwrap(),
            end_time: None,
            turns: vec![Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: "test".to_string(),
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            }],
            total_tokens: TokenUsage::default(),
        }
    }
}
