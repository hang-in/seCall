use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MODEL_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json";
const HF_API_URL: &str = "https://huggingface.co/api/models/BAAI/bge-m3";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub model: String,
    pub downloaded_at: String,
    pub sha256_model: String,
    pub sha256_tokenizer: String,
    pub source_revision: String,
}

#[derive(Debug)]
pub struct ModelInfo {
    pub path: PathBuf,
    pub version: Option<VersionInfo>,
    pub model_size: Option<u64>,
    pub tokenizer_size: Option<u64>,
}

pub enum UpdateStatus {
    UpToDate,
    NeedsUpdate { remote_modified: String },
    NotInstalled,
    CheckFailed(String),
}

pub struct ModelManager {
    model_dir: PathBuf,
    client: Client,
}

impl ModelManager {
    pub fn new(model_dir: PathBuf) -> Self {
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap_or_default();
        ModelManager { model_dir, client }
    }

    pub fn is_downloaded(&self) -> bool {
        self.model_dir.join("model.onnx").exists() && self.model_dir.join("tokenizer.json").exists()
    }

    pub async fn download(&self, force: bool) -> Result<()> {
        if self.is_downloaded() && !force {
            tracing::info!("model already exists, use --force to re-download");
            return Ok(());
        }
        std::fs::create_dir_all(&self.model_dir).context("failed to create model directory")?;

        let model_sha = self
            .download_file(MODEL_URL, "model.onnx")
            .await
            .context("failed to download model.onnx")?;

        let tokenizer_sha = self
            .download_file(TOKENIZER_URL, "tokenizer.json")
            .await
            .context("failed to download tokenizer.json")?;

        let version = VersionInfo {
            model: "BAAI/bge-m3".to_string(),
            downloaded_at: chrono::Utc::now().to_rfc3339(),
            sha256_model: model_sha,
            sha256_tokenizer: tokenizer_sha,
            source_revision: "main".to_string(),
        };
        let version_path = self.model_dir.join("version.json");
        std::fs::write(&version_path, serde_json::to_string_pretty(&version)?)
            .context("failed to write version.json")?;

        tracing::info!(path = %self.model_dir.display(), "model downloaded");
        Ok(())
    }

    async fn download_file(&self, url: &str, final_name: &str) -> Result<String> {
        use futures_util::StreamExt;
        use std::io::Write;

        let tmp_path = self.model_dir.join(format!("{final_name}.tmp"));
        let final_path = self.model_dir.join(final_name);

        let resp = self
            .client
            .get(url)
            .send()
            .await
            .context("HTTP request failed")?;

        if !resp.status().is_success() {
            return Err(anyhow!("download failed ({}): {}", resp.status(), url));
        }

        let total = resp.content_length();
        let mut stream = resp.bytes_stream();

        let mut file = std::fs::File::create(&tmp_path).context("failed to create temp file")?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("download stream error")?;
            hasher.update(&chunk);
            file.write_all(&chunk).context("write failed")?;
            downloaded += chunk.len() as u64;

            if let Some(total) = total {
                let pct = downloaded * 100 / total;
                eprint!(
                    "\r⬇ Downloading {final_name}... {pct}% ({}/{})",
                    format_bytes(downloaded),
                    format_bytes(total)
                );
            } else {
                eprint!(
                    "\r⬇ Downloading {final_name}... {}",
                    format_bytes(downloaded)
                );
            }
        }
        tracing::info!(name = final_name, size = %format_bytes(downloaded), "download complete");

        drop(file);
        std::fs::rename(&tmp_path, &final_path).context("failed to rename temp file")?;

        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn check_update(&self) -> Result<UpdateStatus> {
        if !self.is_downloaded() {
            return Ok(UpdateStatus::NotInstalled);
        }

        let resp = self.client.get(HF_API_URL).send().await;

        match resp {
            Err(e) => Ok(UpdateStatus::CheckFailed(e.to_string())),
            Ok(r) if !r.status().is_success() => {
                Ok(UpdateStatus::CheckFailed(format!("HTTP {}", r.status())))
            }
            Ok(r) => {
                let json: serde_json::Value = r.json().await?;
                let remote_modified = json
                    .get("lastModified")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let version_path = self.model_dir.join("version.json");
                if let Ok(content) = std::fs::read_to_string(&version_path) {
                    if let Ok(local_ver) = serde_json::from_str::<VersionInfo>(&content) {
                        if local_ver.downloaded_at >= remote_modified {
                            return Ok(UpdateStatus::UpToDate);
                        }
                    }
                }
                Ok(UpdateStatus::NeedsUpdate { remote_modified })
            }
        }
    }

    pub fn remove(&self) -> Result<()> {
        if self.model_dir.exists() {
            std::fs::remove_dir_all(&self.model_dir).context("failed to remove model directory")?;
            tracing::info!(path = %self.model_dir.display(), "model removed");
        } else {
            tracing::warn!("model directory not found");
        }
        Ok(())
    }

    pub fn info(&self) -> Result<ModelInfo> {
        let version = {
            let path = self.model_dir.join("version.json");
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                serde_json::from_str::<VersionInfo>(&content).ok()
            } else {
                None
            }
        };

        let model_size = std::fs::metadata(self.model_dir.join("model.onnx"))
            .ok()
            .map(|m| m.len());
        let tokenizer_size = std::fs::metadata(self.model_dir.join("tokenizer.json"))
            .ok()
            .map(|m| m.len());

        Ok(ModelInfo {
            path: self.model_dir.clone(),
            version,
            model_size,
            tokenizer_size,
        })
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.0}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.0}KB", bytes as f64 / 1024.0)
    }
}

pub fn default_model_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("secall")
        .join("models")
        .join("bge-m3-onnx")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_manager_not_downloaded() {
        let dir = TempDir::new().unwrap();
        let mgr = ModelManager::new(dir.path().to_path_buf());
        assert!(!mgr.is_downloaded());
    }

    #[test]
    fn test_version_json_serde() {
        let v = VersionInfo {
            model: "BAAI/bge-m3".to_string(),
            downloaded_at: "2026-04-06T12:00:00Z".to_string(),
            sha256_model: "abc123".to_string(),
            sha256_tokenizer: "def456".to_string(),
            source_revision: "main".to_string(),
        };
        let json = serde_json::to_string(&v).unwrap();
        let v2: VersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(v.model, v2.model);
        assert_eq!(v.sha256_model, v2.sha256_model);
    }

    #[test]
    fn test_default_model_path() {
        let path = default_model_path();
        assert!(path.to_str().unwrap().contains("bge-m3-onnx"));
    }

    #[test]
    #[ignore]
    fn test_download_real() {
        // Manual: requires network
        let dir = TempDir::new().unwrap();
        let mgr = ModelManager::new(dir.path().to_path_buf());
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(mgr.download(false)).unwrap();
        assert!(mgr.is_downloaded());
    }
}
