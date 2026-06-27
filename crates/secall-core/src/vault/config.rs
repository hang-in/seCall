use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::llm::defaults::{GRAPH_ANTHROPIC_DEFAULT, GRAPH_OLLAMA_DEFAULT};

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
    /// Embedding backend: "ollama" | "ort" | "openai" | "openvino" | "ollama_cloud"
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
    /// ORT session pool size. None = auto-detect from RAM (≤15GB→1, 16-31GB→2, ≥32GB→4)
    pub pool_size: Option<usize>,
    /// Ollama Cloud API host (ollama_cloud backend)
    pub cloud_host: Option<String>,
    /// Ollama Cloud embedding model name (ollama_cloud backend)
    pub cloud_model: Option<String>,
    /// Ollama Cloud API key — managed via env OLLAMA_CLOUD_API_KEY, not stored in config
    pub cloud_api_key: Option<String>,
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
    /// P56: ollama_cloud backend 의 bearer auth 키. 없으면 OLLAMA_CLOUD_API_KEY env
    /// 또는 graph/log 의 cloud_api_key 로 fallback.
    pub cloud_api_key: Option<String>,
    /// P56: ollama_cloud backend 의 base URL (default `https://ollama.com`).
    pub cloud_host: Option<String>,
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
            cloud_api_key: None,
            cloud_host: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WikiConfig {
    /// 기본 사용 백엔드: "codex"(기본) | "claude" | "haiku" | "ollama" | "ollama_cloud" | "lmstudio"
    #[serde(default = "default_wiki_backend")]
    pub default_backend: String,
    /// 백엔드별 설정 맵
    #[serde(default)]
    pub backends: std::collections::HashMap<String, WikiBackendConfig>,
    /// Review backend name. None이면 default_backend, 그마저도 불명확하면 "haiku".
    #[serde(default)]
    pub review_backend: Option<String>,
    /// --review 시 사용할 모델: "sonnet" | "opus" (기본: sonnet)
    #[serde(default)]
    pub review_model: Option<String>,
    /// P85 (issue #87): wiki generation backend timeout in seconds.
    /// claude/codex/ollama/lmstudio backend 의 generate() 가 사용한다.
    /// 기본값 1800 (30분) — P59 에서 5분 → 30분 상향한 hardcoded 값과 동일.
    /// 수만 세션 vault 또는 느린 모델 사용 시 사용자가 더 큰 값으로 override 가능.
    #[serde(default = "default_wiki_generation_timeout_secs")]
    pub generation_timeout_secs: u64,
}

fn default_wiki_backend() -> String {
    "codex".to_string()
}

/// Gemini PR #90 리뷰: compile-time const 로 정의하여 명확성/유지보수성 향상.
/// serde default 속성용 함수도 같은 const 를 반환한다 (단일 source of truth).
const DEFAULT_WIKI_GENERATION_TIMEOUT_SECS: u64 = 1800;

fn default_wiki_generation_timeout_secs() -> u64 {
    DEFAULT_WIKI_GENERATION_TIMEOUT_SECS
}

impl Default for WikiConfig {
    fn default() -> Self {
        WikiConfig {
            default_backend: default_wiki_backend(),
            backends: std::collections::HashMap::new(),
            review_backend: None,
            review_model: None,
            generation_timeout_secs: DEFAULT_WIKI_GENERATION_TIMEOUT_SECS,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct GraphConfig {
    /// 시맨틱 엣지 추출 활성화 (기본: true)
    pub semantic: bool,
    /// LLM backend: "ollama" (기본) | "anthropic" | "ollama_cloud" | "lmstudio" | "disabled" (규칙 기반만)
    pub semantic_backend: String,
    /// Ollama base URL (ollama backend)
    pub ollama_url: Option<String>,
    /// Ollama model name (ollama backend, 기본: gemma4:e4b)
    pub ollama_model: Option<String>,
    /// Anthropic model name (anthropic backend, 기본: claude-haiku-4-5-20251001)
    pub anthropic_model: Option<String>,
    /// Ollama Cloud base URL (ollama_cloud backend, 기본: https://ollama.com)
    pub cloud_host: Option<String>,
    /// Ollama Cloud model (ollama_cloud backend, 기본: GRAPH_OLLAMA_CLOUD_DEFAULT)
    pub cloud_model: Option<String>,
    /// Ollama Cloud API key (config field; env OLLAMA_CLOUD_API_KEY 우선)
    pub cloud_api_key: Option<String>,
}

impl Default for GraphConfig {
    fn default() -> Self {
        GraphConfig {
            semantic: true,
            // P51: cloud 우선 (`OLLAMA_CLOUD_API_KEY` 또는 `[graph].cloud_api_key`
            // 가 설정돼 있어야 동작). 키 없으면 호출 시 명시 에러로 실패한다.
            // local 강제 시 config 에 `semantic_backend = "ollama"` 명시.
            semantic_backend: "ollama_cloud".to_string(),
            ollama_url: None,
            ollama_model: None,
            anthropic_model: None,
            cloud_host: None,
            cloud_model: None,
            cloud_api_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct LogConfig {
    /// Daily log backend.
    ///
    /// P51: 디폴트는 `Some("ollama_cloud")` — cloud 우선. None 이면 호출자가
    /// `graph.semantic_backend` 로 폴백한다. config 에 명시한 값이 있으면 그것을
    /// 사용한다.
    pub backend: Option<String>,
    /// Model override for the selected log backend
    pub model: Option<String>,
    /// API base URL override for ollama / lmstudio
    pub api_url: Option<String>,
    /// Max generation tokens override
    pub max_tokens: Option<u32>,
    /// Ollama Cloud base URL override (ollama_cloud backend)
    pub cloud_host: Option<String>,
    /// Ollama Cloud model override (ollama_cloud backend)
    pub cloud_model: Option<String>,
    /// Ollama Cloud API key (config field; env OLLAMA_CLOUD_API_KEY 우선)
    pub cloud_api_key: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            // P51: cloud 우선. `OLLAMA_CLOUD_API_KEY` 가 설정돼 있어야 실 동작.
            backend: Some("ollama_cloud".to_string()),
            model: None,
            api_url: None,
            max_tokens: None,
            cloud_host: None,
            cloud_model: None,
            cloud_api_key: None,
        }
    }
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
            pool_size: None,
            cloud_host: None,
            cloud_model: None,
            cloud_api_key: None,
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
                "anthropic" => self.graph.anthropic_model = Some(m),
                "ollama_cloud" => self.graph.cloud_model = Some(m),
                _ => self.graph.ollama_model = Some(m),
            }
        }
        if let Ok(k) = std::env::var("OLLAMA_CLOUD_API_KEY") {
            self.graph.cloud_api_key = Some(k.clone());
            self.log.cloud_api_key = Some(k.clone());
            self.embedding.cloud_api_key = Some(k);
        }
        self
    }

    pub fn save(&self) -> Result<()> {
        use anyhow::Context as _;

        let path = Self::config_path();

        // P68 + P82: test 환경에서 SECALL_CONFIG_PATH 미설정 시 production
        // config 를 덮어쓰는 사고 방지. P68 은 unit test 만 (#[cfg(test)]),
        // P82 에서 runtime env (`SECALL_TEST_MODE=1`) 로 integration test 까지
        // 확장. integration test 는 `tests/common::ensure_test_mode()` 호출로
        // SECALL_TEST_MODE 를 set 한다.
        let in_test = cfg!(test) || std::env::var("SECALL_TEST_MODE").is_ok();
        if in_test && std::env::var("SECALL_CONFIG_PATH").is_err() {
            anyhow::bail!(
                "Config::save() called without SECALL_CONFIG_PATH in test \
                 context (cfg!(test) 또는 SECALL_TEST_MODE 감지) — refusing \
                 to write production config at {:?}. 테스트는 \
                 SECALL_CONFIG_PATH 를 tempdir 로 set 한 후 save() 를 \
                 호출해야 합니다.",
                path
            );
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut doc = if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            raw.parse::<toml_edit::DocumentMut>()
                .context("existing config.toml is invalid")?
        } else {
            toml_edit::DocumentMut::new()
        };
        merge_into_doc(&mut doc, self)?;
        let tmp_path = path.with_extension(format!(
            "toml.tmp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        std::fs::write(&tmp_path, doc.to_string())?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

fn merge_into_doc(doc: &mut toml_edit::DocumentMut, config: &Config) -> Result<()> {
    sync_section(doc, "vault", &config.vault)?;
    sync_section(doc, "ingest", &config.ingest)?;
    sync_section(doc, "search", &config.search)?;
    sync_section(doc, "hooks", &config.hooks)?;
    sync_section(doc, "embedding", &config.embedding)?;
    sync_section(doc, "openvino", &config.openvino)?;
    sync_section(doc, "output", &config.output)?;
    sync_section(doc, "wiki", &config.wiki)?;
    sync_section(doc, "graph", &config.graph)?;
    sync_section(doc, "log", &config.log)?;
    Ok(())
}

fn sync_section<T: serde::Serialize>(
    doc: &mut toml_edit::DocumentMut,
    section_name: &str,
    section: &T,
) -> Result<()> {
    use anyhow::Context as _;

    let table = doc[section_name]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .with_context(|| format!("[{section_name}] is not a table"))?;
    let serialized = toml::Value::try_from(section)?;
    let map = serialized
        .as_table()
        .with_context(|| format!("[{section_name}] failed to serialize as table"))?;
    sync_table(table, map)
}

fn sync_table(
    table: &mut toml_edit::Table,
    map: &toml::map::Map<String, toml::Value>,
) -> Result<()> {
    let to_remove: Vec<String> = table
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| !map.contains_key(key))
        .collect();
    for key in to_remove {
        table.remove(&key);
    }

    for (key, value) in map {
        match value {
            toml::Value::Table(inner) => sync_nested_table(table, key, inner)?,
            _ => sync_value_item(table, key, value)?,
        }
    }

    Ok(())
}

fn sync_nested_table(
    table: &mut toml_edit::Table,
    key: &str,
    map: &toml::map::Map<String, toml::Value>,
) -> Result<()> {
    let implicit = map
        .values()
        .all(|value| matches!(value, toml::Value::Table(_)));

    if let Some(existing) = table.get_mut(key) {
        if let Some(existing_table) = existing.as_table_mut() {
            existing_table.set_implicit(implicit);
            return sync_table(existing_table, map);
        }
    }

    let mut new_table = toml_edit::Table::new();
    new_table.set_implicit(implicit);
    sync_table(&mut new_table, map)?;
    table.insert(key, toml_edit::Item::Table(new_table));
    Ok(())
}

fn sync_value_item(table: &mut toml_edit::Table, key: &str, value: &toml::Value) -> Result<()> {
    let new_value = toml_value_to_value(value)?;

    if let Some(existing) = table.get_mut(key) {
        if let Some(existing_value) = existing.as_value_mut() {
            let decor = existing_value.decor().clone();
            let mut replacement = new_value;
            replacement
                .decor_mut()
                .set_prefix(decor.prefix().cloned().unwrap_or_default());
            replacement
                .decor_mut()
                .set_suffix(decor.suffix().cloned().unwrap_or_default());
            *existing_value = replacement;
            return Ok(());
        }
    }

    table.insert(key, toml_edit::Item::Value(new_value));
    Ok(())
}

fn toml_value_to_value(value: &toml::Value) -> Result<toml_edit::Value> {
    use anyhow::Context as _;

    match value {
        toml::Value::Table(_) => {
            anyhow::bail!("nested table cannot be converted into a scalar value")
        }
        _ => {
            let raw = value.to_string();
            raw.parse::<toml_edit::Value>()
                .with_context(|| format!("failed to convert TOML value: {raw}"))
        }
    }
}

pub fn default_graph_ollama_model() -> &'static str {
    GRAPH_OLLAMA_DEFAULT
}

pub fn default_graph_anthropic_model() -> &'static str {
    GRAPH_ANTHROPIC_DEFAULT
}

/// 환경변수(`SECALL_CONFIG_PATH`, `OLLAMA_CLOUD_API_KEY` 등)를 변경하는 테스트들이
/// 같은 lib binary 내에서 병렬 실행될 때 서로 간섭하지 않도록 직렬화한다.
///
/// `vault::config::tests` 외 `vault::tests::test_config_load_or_default` 처럼 다른
/// 모듈에서도 동일 env 를 건드리므로 `pub(crate)` 로 노출해 공유한다.
#[cfg(test)]
pub(crate) static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    #[test]
    fn test_graph_env_override_backend() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_BACKEND", "ollama_cloud");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_BACKEND");
        assert_eq!(config.graph.semantic_backend, "ollama_cloud");
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
    fn test_graph_env_override_model_ollama_cloud() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("SECALL_GRAPH_BACKEND", "ollama_cloud");
        std::env::set_var("SECALL_GRAPH_MODEL", "gemma4:custom");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("SECALL_GRAPH_BACKEND");
        std::env::remove_var("SECALL_GRAPH_MODEL");
        // ollama_cloud 일 때 SECALL_GRAPH_MODEL → cloud_model 에 저장돼야 함
        assert_eq!(config.graph.cloud_model, Some("gemma4:custom".to_string()));
        // ollama_model 에는 저장되지 않아야 함
        assert_eq!(config.graph.ollama_model, None);
    }

    #[test]
    fn test_ollama_cloud_api_key_env_override() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::set_var("OLLAMA_CLOUD_API_KEY", "test-cloud-key");
        let config = Config::default().apply_env_overrides();
        std::env::remove_var("OLLAMA_CLOUD_API_KEY");
        assert_eq!(
            config.graph.cloud_api_key,
            Some("test-cloud-key".to_string())
        );
        assert_eq!(config.log.cloud_api_key, Some("test-cloud-key".to_string()));
        assert_eq!(
            config.embedding.cloud_api_key,
            Some("test-cloud-key".to_string()),
            "OLLAMA_CLOUD_API_KEY should propagate to embedding.cloud_api_key"
        );
    }

    #[test]
    fn test_embedding_pool_size_round_trip() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[vault]
path = "/tmp/test"

[embedding]
pool_size = 2
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);
        let config = Config::load_or_default();
        std::env::remove_var("SECALL_CONFIG_PATH");
        assert_eq!(
            config.embedding.pool_size,
            Some(2),
            "pool_size should round-trip as Some(2)"
        );
    }

    #[test]
    fn test_embedding_pool_size_default_is_none() {
        let config = Config::default();
        assert_eq!(
            config.embedding.pool_size, None,
            "pool_size default should be None (auto-detect)"
        );
    }

    #[test]
    fn save_preserves_top_level_comments() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"# Top-level note: this is the user's comment.
# Multiple lines.

[vault]
path = "/tmp/test"
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.vault.path = "/tmp/changed".into();
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# Top-level note: this is the user's comment."));
        assert!(saved.contains("# Multiple lines."));
        assert!(saved.contains(r#"path = "/tmp/changed""#));
    }

    #[test]
    fn save_preserves_inline_comments() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[vault]
path = "/tmp/test" # keep me
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.vault.path = "/tmp/changed".into();
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("# keep me"));
        assert!(saved.contains(r#"path = "/tmp/changed""#));
    }

    #[test]
    fn save_writes_new_keys_in_existing_section() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[vault]
path = "/tmp/test"

[wiki]
default_backend = "claude"
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.wiki.review_backend = Some("ollama".into());
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("[wiki]"));
        assert!(saved.contains(r#"review_backend = "ollama""#));
    }

    #[test]
    fn save_removes_optional_keys_when_cleared() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[vault]
path = "/tmp/test"

[wiki]
default_backend = "claude"
review_backend = "ollama"
review_model = "sonnet"
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.wiki.review_backend = None;
        config.wiki.review_model = None;
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("review_backend"));
        assert!(!saved.contains("review_model"));
        assert!(saved.contains(r#"default_backend = "claude""#));
    }

    #[test]
    fn save_preserves_nested_tables_for_wiki_backends() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[vault]
path = "/tmp/test"

[wiki]
default_backend = "ollama"

[wiki.backends.ollama]
api_url = "http://localhost:11434"
model = "llama3.1"
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.wiki.review_backend = Some("ollama".into());
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("[wiki.backends.ollama]"));
        assert!(!saved.contains("backends = {"));
        assert!(saved.contains(r#"review_backend = "ollama""#));
    }

    #[test]
    fn save_creates_new_section_when_absent() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[vault]
path = "/tmp/test"
"#,
        )
        .unwrap();
        std::env::set_var("SECALL_CONFIG_PATH", &path);

        let mut config = Config::load_or_default();
        config.log.backend = Some("ollama".into());
        config.save().unwrap();
        std::env::remove_var("SECALL_CONFIG_PATH");

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("[log]"));
        assert!(saved.contains(r#"backend = "ollama""#));
        assert!(saved.contains(r#"path = "/tmp/test""#));
    }

    /// P68: test 환경에서 `SECALL_CONFIG_PATH` 미설정 시 `Config::save()` 가
    /// production config 를 덮어쓰지 못하도록 가드. 2026-05-16 사고 회귀 방지.
    #[test]
    fn save_refuses_in_test_context_without_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // 다른 테스트의 잔여 set 이 있을 수 있어 명시적 unset.
        std::env::remove_var("SECALL_CONFIG_PATH");
        let config = Config::default();
        let result = config.save();
        assert!(
            result.is_err(),
            "save() must refuse without SECALL_CONFIG_PATH in test context"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("SECALL_CONFIG_PATH"),
            "error must mention SECALL_CONFIG_PATH, got: {msg}"
        );
    }
}
