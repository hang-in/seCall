use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

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

        // P58: claude stdout 을 line stream 으로 stderr 에 echo + buffer 누적.
        // 이전 (P52) 의 wait_with_output 은 모든 출력이 모이고 나서야 사용자가
        // 봤음 → 5분 timeout 동안 사용자는 "아무 반응 없음" 으로 인식 + Ctrl+C
        // 유혹. 이제 매 line 받는 즉시 `[claude]` prefix 로 stderr 에 echo,
        // 동시에 buffer 에 누적해 wiki page 본문으로 반환.
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("claude stdout pipe missing"))?;
        let mut reader = BufReader::new(stdout).lines();

        // P52: 300s timeout 유지. kill_on_drop=true 라 timeout 시 자동 SIGKILL.
        let stream_and_wait = async {
            let mut buf = String::new();
            while let Some(line) = reader.next_line().await? {
                eprintln!("  [claude] {}", line);
                buf.push_str(&line);
                buf.push('\n');
            }
            let status = child.wait().await?;
            Ok::<_, anyhow::Error>((status, buf))
        };

        let (status, buffer) =
            tokio::time::timeout(std::time::Duration::from_secs(300), stream_and_wait)
                .await
                .map_err(|_| anyhow::anyhow!("claude wiki generation timed out after 300s"))??;

        if !status.success() {
            anyhow::bail!("claude exited with code {:?}", status.code());
        }
        Ok(buffer)
    }
}
