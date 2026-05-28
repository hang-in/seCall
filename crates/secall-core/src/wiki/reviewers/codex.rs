use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt as _;

use crate::wiki::{load_review_system_prompt, ReviewResult, ReviewerKind, WikiReviewer};

pub struct CodexReviewer {
    pub model: String,
    pub vault_path: PathBuf,
}

#[async_trait]
impl WikiReviewer for CodexReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        if !crate::command_exists("codex") {
            anyhow::bail!("codex CLI not found in PATH");
        }

        // Gemini PR #96: PATH 탐색 (disk I/O) 은 루프 밖에서 1회만.
        let program = crate::resolve_program("codex");

        for strict in [false, true] {
            let prompt = format!(
                "{}\n\n{}",
                load_review_system_prompt(ReviewerKind::Codex),
                super::build_user_prompt(page_content, source_summary, strict)
            );

            let output_file = tempfile::NamedTempFile::new()?;
            let output_path = output_file.path().to_path_buf();

            let mut child = tokio::process::Command::new(&program);
            child
                .args([
                    "exec",
                    "--skip-git-repo-check",
                    "--sandbox",
                    "workspace-write",
                    "-C",
                ])
                .arg(&self.vault_path)
                .args(["-m", &self.model, "--output-last-message"])
                .arg(&output_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .kill_on_drop(true);

            let mut child = child.spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(prompt.as_bytes()).await?;
            }

            let status = tokio::time::timeout(std::time::Duration::from_secs(60), child.wait())
                .await
                .map_err(|_| anyhow::anyhow!("codex review timed out after 60s"))??;

            if !status.success() {
                anyhow::bail!("codex exited with code {:?}", status.code());
            }

            let stdout = std::fs::read_to_string(&output_path)?;
            if let Ok(result) = super::parse_review_response(&stdout) {
                return Ok(result);
            }
        }

        anyhow::bail!("review JSON parse failed after retry")
    }
}
