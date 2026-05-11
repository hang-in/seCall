use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt as _;

use super::WikiBackend;

pub struct ClaudeBackend {
    pub model: String,
    pub vault_path: PathBuf,
}

#[async_trait]
impl WikiBackend for ClaudeBackend {
    fn name(&self) -> &'static str {
        "claude"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        if !crate::command_exists("claude") {
            anyhow::bail!(
                "Claude Code CLI not found in PATH. \
                 Install: https://docs.anthropic.com/claude-code"
            );
        }

        let model_id = match self.model.as_str() {
            "opus" => "claude-opus-4-6",
            _ => "claude-sonnet-4-6",
        };

        let mut child = tokio::process::Command::new("claude")
            .args(["-p", "--model", model_id])
            .arg("--allowedTools")
            .arg("mcp__secall__recall,mcp__secall__get,mcp__secall__status,mcp__secall__wiki_search,Read,Write,Edit,Glob,Grep")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .current_dir(&self.vault_path)
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = child.wait_with_output().await?;
        if !output.status.success() {
            anyhow::bail!("claude exited with code {:?}", output.status.code());
        }

        String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("claude stdout was not UTF-8: {e}"))
    }
}
