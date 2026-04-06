use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub vault: VaultConfig,
    pub ingest: IngestConfig,
    pub search: SearchConfig,
    pub hooks: HooksConfig,
    pub embedding: EmbeddingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub git_remote: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct IngestConfig {
    pub tool_output_max_chars: usize,
    pub thinking_included: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SearchConfig {
    pub default_limit: usize,
    /// Tokenizer backend: "lindera" | "kiwi"
    pub tokenizer: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Embedding backend: "ollama" | "ort" | "openai"
    pub backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend)
    pub ollama_model: Option<String>,
    /// ONNX model directory (ort backend)
    pub model_path: Option<PathBuf>,
    /// OpenAI model name (openai backend)
    pub openai_model: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HooksConfig {
    pub post_ingest: Option<String>,
    pub hook_timeout_secs: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            vault: VaultConfig {
                path: dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("obsidian-vault")
                    .join("seCall"),
                git_remote: None,
            },
            ingest: IngestConfig::default(),
            search: SearchConfig::default(),
            hooks: HooksConfig::default(),
            embedding: EmbeddingConfig::default(),
        }
    }
}

impl Default for IngestConfig {
    fn default() -> Self {
        IngestConfig {
            tool_output_max_chars: 500,
            thinking_included: true,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            default_limit: 10,
            tokenizer: "lindera".to_string(), // existing behavior
        }
    }
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        EmbeddingConfig {
            backend: "ollama".to_string(), // existing behavior
            ollama_url: None,
            ollama_model: None,
            model_path: None,
            openai_model: None,
        }
    }
}

impl Default for VaultConfig {
    fn default() -> Self {
        VaultConfig {
            path: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("obsidian-vault")
                .join("seCall"),
            git_remote: None,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        if let Ok(p) = std::env::var("SECALL_CONFIG_PATH") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("secall")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        // Apply env override
        let config = config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_default().apply_env_overrides()
    }

    fn apply_env_overrides(mut self) -> Self {
        if let Ok(p) = std::env::var("SECALL_VAULT_PATH") {
            self.vault.path = PathBuf::from(p);
        }
        self
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
