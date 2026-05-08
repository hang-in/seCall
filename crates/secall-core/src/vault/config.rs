use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::llm::defaults::{
    GRAPH_ANTHROPIC_DEFAULT, GRAPH_GEMINI_DEFAULT, GRAPH_OLLAMA_DEFAULT,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub vault: VaultConfig,
    pub ingest: IngestConfig,
    pub search: SearchConfig,
    pub hooks: HooksConfig,
    pub embedding: EmbeddingConfig,
    pub openvino: OpenVinoConfig,
    pub output: OutputConfig,
    pub wiki: WikiConfig,
    pub graph: GraphConfig,
    pub log: LogConfig,
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
    /// Embedding backend: "ollama" | "ort" | "openai" | "openvino"
    pub backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend)
    pub ollama_model: Option<String>,
    /// ONNX model directory (ort / openvino backend)
    pub model_path: Option<PathBuf>,
    /// OpenAI model name (openai backend)
    pub openai_model: Option<String>,
    /// OpenVINO device type: "NPU" | "GPU" | "CPU" (openvino backend)
    pub openvino_device: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct OpenVinoConfig {
    /// Path to OpenVINO installation directory (sets INTEL_OPENVINO_DIR)
    pub dir: Option<String>,
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
    /// 모델 이름 (backend별 기본값: claude=sonnet, codex=gpt-5.4)
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
    /// 기본 사용 백엔드: "claude" | "haiku" | "ollama" | "lmstudio"
    #[serde(default = "default_wiki_backend")]
    pub default_backend: String,
    /// 백엔드별 설정 맵
    #[serde(default)]
    pub backends: std::collections::HashMap<String, WikiBackendConfig>,
    /// --review 시 사용할 모델: "sonnet" | "opus" (기본: sonnet)
    #[serde(default)]
    pub review_model: Option<String>,
}

fn default_wiki_backend() -> String {
    "claude".to_string()
}

impl Default for WikiConfig {
    fn default() -> Self {
        WikiConfig {
            default_backend: default_wiki_backend(),
            backends: std::collections::HashMap::new(),
            review_model: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GraphConfig {
    /// 시맨틱 엣지 추출 활성화 (기본: true)
    pub semantic: bool,
    /// LLM backend: "ollama" (기본) | "anthropic" | "gemini" | "lmstudio" | "disabled" (규칙 기반만)
    pub semantic_backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend, 기본: gemma4:e4b)
    pub ollama_model: Option<String>,
    /// Anthropic model name (anthropic backend, 기본: claude-haiku-4-5-20251001)
    pub anthropic_model: Option<String>,
    /// Gemini API key (gemini backend, fallback: SECALL_GEMINI_API_KEY 환경변수)
    pub gemini_api_key: Option<String>,
    /// Gemini model name (gemini backend, 기본: gemini-2.5-flash)
    pub gemini_model: Option<String>,
}

impl Default for GraphConfig {
    fn default() -> Self {
        GraphConfig {
            semantic: true,
            semantic_backend: "ollama".to_string(),
            ollama_url: None,
            ollama_model: None,
            anthropic_model: None,
            gemini_api_key: None,
            gemini_model: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LogConfig {
    /// Daily log backend (None이면 graph.semantic_backend → "ollama" 폴백)
    pub backend: Option<String>,
    /// Model override for the selected log backend
    pub model: Option<String>,
    /// API base URL override for ollama / lmstudio
    pub api_url: Option<String>,
    /// Max generation tokens override
    pub max_tokens: Option<u32>,
}

/// 단일 세션 분류 규칙
/// pattern 또는 project 중 하나 이상 지정해야 함.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClassificationRule {
    /// 첫 번째 user turn 내용에 매칭할 regex 패턴 (선택)
    #[serde(default)]
    pub pattern: Option<String>,
    /// 세션의 project 필드와 정확히 일치할 프로젝트명 (선택)
    #[serde(default)]
    pub project: Option<String>,
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
            openvino: OpenVinoConfig::default(),
            output: OutputConfig::default(),
            wiki: WikiConfig::default(),
            graph: GraphConfig::default(),
            log: LogConfig::default(),
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
            openvino_device: None,
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
        // Graph semantic 환경변수 (CLI 플래그보다 낮은 우선순위)
        if let Ok(b) = std::env::var("SECALL_GRAPH_BACKEND") {
            self.graph.semantic_backend = b;
        }
        if let Ok(u) = std::env::var("SECALL_GRAPH_API_URL") {
            self.graph.ollama_url = Some(u);
        }
        if let Ok(m) = std::env::var("SECALL_GRAPH_MODEL") {
            match self.graph.semantic_backend.as_str() {
                "gemini" => self.graph.gemini_model = Some(m),
                "anthropic" => self.graph.anthropic_model = Some(m),
                _ => self.graph.ollama_model = Some(m),
            }
        }
        if let Ok(k) = std::env::var("SECALL_GRAPH_API_KEY") {
            self.graph.gemini_api_key = Some(k);
        }
        self
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        let tmp_path = path.with_extension(format!(
            "toml.tmp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

pub fn default_graph_ollama_model() -> &'static str {
    GRAPH_OLLAMA_DEFAULT
}

pub fn default_graph_anthropic_model() -> &'static str {
    GRAPH_ANTHROPIC_DEFAULT
}

pub fn default_graph_gemini_model() -> &'static str {
    GRAPH_GEMINI_DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 환경변수를 변경하는 테스트들이 병렬 실행될 때 서로 간섭하지 않도록 직렬화
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    #[test]
    fn test_graph_env_override_backend() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_BACKEND", "gemini");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_BACKEND");
        assert_eq!(config.graph.semantic_backend, "gemini");
    }

    #[test]
    fn test_graph_env_override_api_url() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_API_URL", "http://custom:8080");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_API_URL");
        assert_eq!(
            config.graph.ollama_url,
            Some("http://custom:8080".to_string())
        );
    }

    #[test]
    fn test_graph_env_override_model_gemini() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_BACKEND", "gemini");
        std::env::set_var("SECALL_GRAPH_MODEL", "gemini-2.0-flash");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_BACKEND");
        std::env::remove_var("SECALL_GRAPH_MODEL");
        assert_eq!(
            config.graph.gemini_model,
            Some("gemini-2.0-flash".to_string())
        );
    }

    #[test]
    fn test_graph_env_override_api_key() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_API_KEY", "test-key-123");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_API_KEY");
        assert_eq!(
            config.graph.gemini_api_key,
            Some("test-key-123".to_string())
        );
    }
}
