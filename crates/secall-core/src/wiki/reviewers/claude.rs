use std::path::PathBuf;
use std::process::Stdio;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt as _;

use crate::wiki::{load_review_system_prompt, ReviewResult, ReviewerKind, WikiReviewer};

pub struct ClaudeReviewer {
    pub model: String,
    pub vault_path: PathBuf,
}

#[async_trait]
impl WikiReviewer for ClaudeReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        run_review_cli(
            "claude",
            &["-p", "--model", &self.model],
            &self.vault_path,
            ReviewerKind::Claude,
            page_content,
            source_summary,
        )
        .await
    }
}

async fn run_review_cli(
    bin: &str,
    args: &[&str],
    cwd: &std::path::Path,
    kind: ReviewerKind,
    page_content: &str,
    source_summary: &str,
) -> Result<ReviewResult> {
    if !crate::command_exists(bin) {
        anyhow::bail!("{bin} CLI not found in PATH");
    }

    for strict in [false, true] {
        let prompt = format!(
            "{}\n\n{}",
            load_review_system_prompt(kind),
            super::build_user_prompt(page_content, source_summary, strict)
        );

        let mut child = tokio::process::Command::new(bin);
        child
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .current_dir(cwd);

        let mut child = child.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
        }

        let output =
            tokio::time::timeout(std::time::Duration::from_secs(60), child.wait_with_output())
                .await
                .map_err(|_| anyhow::anyhow!("{bin} review timed out after 60s"))??;

        if !output.status.success() {
            anyhow::bail!("{bin} exited with code {:?}", output.status.code());
        }

        let stdout = String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("{bin} stdout was not UTF-8: {e}"))?;
        if let Ok(result) = super::parse_review_response(&stdout) {
            return Ok(result);
        }
    }

    anyhow::bail!("review JSON parse failed after retry")
}
