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

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let status = child.wait().await?;
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
