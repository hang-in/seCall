use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt as _;

use super::WikiBackend;

pub struct CodexBackend {
    pub model: String,
    pub vault_path: PathBuf,
}

#[async_trait]
impl WikiBackend for CodexBackend {
    fn name(&self) -> &'static str {
        "codex"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        if !crate::command_exists("codex") {
            anyhow::bail!("Codex CLI not found in PATH. Install: https://github.com/openai/codex");
        }

        let output_file = tempfile::NamedTempFile::new()?;
        let output_path = output_file.path().to_path_buf();

        let mut child = tokio::process::Command::new("codex")
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
            .kill_on_drop(true)
            .spawn()?;

        // P83 (issue #82): prompt 앞에 secall wiki marker 를 prefix 로 추가해
        // codex CLI 가 생성한 세션 파일을 ingest 가 self-ingest 루프로 인식해
        // skip 하도록 한다. 무한 wiki 재생성 차단.
        let marked_prompt = format!("{}\n\n{}", crate::wiki::WIKI_INVOCATION_MARKER, prompt);

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(marked_prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        // P52/P59: codex CLI hang 회피. 1800s 한도 (정상 wiki 생성도 10~20분
        // 걸리는 경우가 있어 P59 에서 5분 → 30분 상향). kill_on_drop=true.
        let status = tokio::time::timeout(std::time::Duration::from_secs(1800), child.wait())
            .await
            .map_err(|_| anyhow::anyhow!("codex wiki generation timed out after 1800s"))??;
        if !status.success() {
            anyhow::bail!("codex exited with code {:?}", status.code());
        }

        let output = std::fs::read_to_string(&output_path)?;
        if output.trim().is_empty() {
            anyhow::bail!("codex returned empty output");
        }

        Ok(output)
    }
}
