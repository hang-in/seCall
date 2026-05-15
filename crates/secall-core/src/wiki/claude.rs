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

        // P56: review default (WIKI_REVIEW_DEFAULT="haiku") 가 claude CLI 에서
        // 의도대로 동작하도록 alias 추가. 이전엔 "haiku" → fallback sonnet 으로
        // 매핑되어 review default 효과 없었음.
        let model_id = match self.model.as_str() {
            "opus" => "claude-opus-4-6",
            "haiku" => "claude-haiku-4-5",
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

        // P52: claude CLI 가 stream / internal lock 으로 hang 하는 사례 회피.
        // wiki 생성은 review 보다 출력이 길어 300s 한도. kill_on_drop=true 라
        // timeout 시 자동 SIGKILL.
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("claude wiki generation timed out after 300s"))??;
        if !output.status.success() {
            anyhow::bail!("claude exited with code {:?}", output.status.code());
        }

        String::from_utf8(output.stdout)
            .map_err(|e| anyhow::anyhow!("claude stdout was not UTF-8: {e}"))
    }
}
