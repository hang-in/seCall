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
    pub output: OutputConfig,
    pub wiki: WikiConfig,
    pub graph: GraphConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OutputConfig {
    /// IANA timezone name (e.g. "Asia/Seoul", "America/New_York")
    /// Default: "UTC"
    pub timezone: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            timezone: "UTC".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub git_remote: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct IngestConfig {
    pub tool_output_max_chars: usize,
    pub thinking_included: bool,
    pub classification: ClassificationConfig,
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

/// 개별 백엔드 설정 (LM Studio, Ollama, Claude 공용)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WikiBackendConfig {
    /// API 엔드포인트 (Claude 백엔드는 사용 안 함)
    pub api_url: Option<String>,
    /// 모델 이름
    pub model: Option<String>,
    /// 최대 생성 토큰 수
    #[serde(default = "default_wiki_max_tokens")]
    pub max_tokens: u32,
}

fn default_wiki_max_tokens() -> u32 {
    4096
}

impl Default for WikiBackendConfig {
    fn default() -> Self {
        WikiBackendConfig {
            api_url: None,
            model: None,
            max_tokens: default_wiki_max_tokens(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WikiConfig {
    /// 기본 사용 백엔드: "claude" | "ollama" | "lmstudio"
    #[serde(default = "default_wiki_backend")]
    pub default_backend: String,
    /// 백엔드별 설정 맵
    #[serde(default)]
    pub backends: std::collections::HashMap<String, WikiBackendConfig>,
}

fn default_wiki_backend() -> String {
    "claude".to_string()
}

impl Default for WikiConfig {
    fn default() -> Self {
        WikiConfig {
            default_backend: default_wiki_backend(),
            backends: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GraphConfig {
    /// 시맨틱 엣지 추출 활성화 (기본: true)
    pub semantic: bool,
    /// LLM backend: "ollama" (기본) | "anthropic" | "disabled" (규칙 기반만)
    pub semantic_backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend, 기본: gemma4:e4b)
    pub ollama_model: Option<String>,
    /// Anthropic model name (anthropic backend, 기본: claude-haiku-4-5-20251001)
    pub anthropic_model: Option<String>,
}

impl Default for GraphConfig {
    fn default() -> Self {
        GraphConfig {
            semantic: true,
            semantic_backend: "ollama".to_string(),
            ollama_url: None,
            ollama_model: None,
            anthropic_model: None,
        }
    }
}

/// 단일 세션 분류 규칙
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClassificationRule {
    /// 첫 번째 user turn 내용에 매칭할 regex 패턴
    pub pattern: String,
    /// 매칭 시 부여할 session_type (예: "automated", "health_check")
    pub session_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ClassificationConfig {
    /// 규칙에 매칭되지 않을 때 기본 session_type
    #[serde(default = "default_session_type")]
    pub default: String,
    /// 순서대로 매칭 시도, 첫 번째 매칭 규칙 적용
    #[serde(default)]
    pub rules: Vec<ClassificationRule>,
    /// 임베딩을 skip할 session_type 목록
    #[serde(default)]
    pub skip_embed_types: Vec<String>,
}

fn default_session_type() -> String {
    "interactive".to_string()
}

impl Default for ClassificationConfig {
    fn default() -> Self {
        ClassificationConfig {
            default: default_session_type(),
            rules: Vec::new(),
            skip_embed_types: Vec::new(),
        }
    }
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
                branch: "main".to_string(),
            },
            ingest: IngestConfig::default(),
            search: SearchConfig::default(),
            hooks: HooksConfig::default(),
            embedding: EmbeddingConfig::default(),
            output: OutputConfig::default(),
            wiki: WikiConfig::default(),
            graph: GraphConfig::default(),
        }
    }
}

impl Default for IngestConfig {
    fn default() -> Self {
        IngestConfig {
            tool_output_max_chars: 500,
            thinking_included: true,
            classification: ClassificationConfig::default(),
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
            branch: "main".to_string(),
        }
    }
}

impl Config {
    /// 특정 백엔드의 설정을 반환한다. 없으면 기본값.
    pub fn wiki_backend_config(&self, name: &str) -> WikiBackendConfig {
        self.wiki.backends.get(name).cloned().unwrap_or_default()
    }

    /// 설정된 타임존을 chrono_tz::Tz로 파싱.
    /// 잘못된 값이면 UTC로 fallback + 경고 로그.
    pub fn timezone(&self) -> chrono_tz::Tz {
        self.output
            .timezone
            .parse::<chrono_tz::Tz>()
            .unwrap_or_else(|_| {
                tracing::warn!(
                    tz = &self.output.timezone,
                    "invalid timezone, falling back to UTC"
                );
                chrono_tz::Tz::UTC
            })
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timezone_default_is_utc() {
        let config = Config::default();
        assert_eq!(config.output.timezone, "UTC");
        assert_eq!(config.timezone(), chrono_tz::Tz::UTC);
    }

    #[test]
    fn test_timezone_valid_iana() {
        let mut config = Config::default();
        config.output.timezone = "Asia/Seoul".to_string();
        assert_eq!(config.timezone(), chrono_tz::Tz::Asia__Seoul);
    }

    #[test]
    fn test_timezone_invalid_falls_back_to_utc() {
        let mut config = Config::default();
        config.output.timezone = "INVALID/TZ".to_string();
        assert_eq!(config.timezone(), chrono_tz::Tz::UTC);
    }

    #[test]
    fn test_config_without_output_section() {
        let toml_str = r#"
[vault]
path = "/tmp/test-vault"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.output.timezone, "UTC");
    }
}
