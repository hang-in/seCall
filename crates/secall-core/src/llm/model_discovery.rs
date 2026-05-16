//! P65 — backend 별 모델 dynamic discovery + cache.
//!
//! tunaFlow 의 `src-tauri/src/commands/model_discovery.rs` 패턴을 따라하되,
//! secall 은 async tokio 환경이므로 `reqwest::Client` (async) 와
//! `tokio::sync::RwLock` 으로 옮긴다.
//!
//! 핵심 원칙:
//! - **Dynamic primary**: 각 backend 의 실 source 에서 fetch
//!   (HTTP `/v1/models`, `/api/tags`, `~/.codex/models_cache.json`,
//!   claude binary scan 등)
//! - **Hardcoded fallback**: discovery 실패 시 fallback list.
//!   fallback 자체도 `llm::defaults` 의 상수와 일관되게 유지한다.
//! - **In-memory cache**: TTL 3600s. claude 만 binary mtime 무효화 추가.
//! - **Force refresh** 옵션.
//!
//! 노출 함수:
//! - [`discover_models`] — async API, [`DiscoveryResult`] 반환.
//! - [`invalidate_cache`] — 단일 backend 또는 전체 cache flush.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use tokio::sync::RwLock;

use crate::llm::defaults::{
    GRAPH_ANTHROPIC_DEFAULT, GRAPH_LMSTUDIO_DEFAULT, GRAPH_OLLAMA_CLOUD_DEFAULT,
    GRAPH_OLLAMA_DEFAULT, LOG_OLLAMA_CLOUD_DEFAULT, WIKI_CODEX_DEFAULT,
    WIKI_REVIEW_OLLAMA_CLOUD_DEFAULT,
};

const CACHE_TTL: Duration = Duration::from_secs(3600);
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

/// discovery 결과 source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoverySource {
    /// 실 backend 에서 가져왔다 (HTTP, file, binary scan 등).
    Dynamic,
    /// dynamic 실패 또는 의도적으로 비활성 — hardcoded fallback list 사용.
    Fallback,
    /// 이전 호출 결과를 cache 에서 재사용.
    Cached,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryResult {
    pub backend: String,
    pub models: Vec<String>,
    pub source: DiscoverySource,
}

#[derive(Clone)]
struct CacheEntry {
    models: Vec<String>,
    at: Instant,
    /// claude binary scan 시 사용. 이외 backend 에서는 None.
    binary_stamp: Option<(PathBuf, SystemTime)>,
}

fn cache() -> &'static RwLock<HashMap<String, CacheEntry>> {
    static CACHE: OnceLock<RwLock<HashMap<String, CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 단일 backend 또는 전체 cache 를 비운다.
pub async fn invalidate_cache(backend: Option<&str>) {
    let mut guard = cache().write().await;
    match backend {
        Some(b) => {
            guard.remove(b);
        }
        None => guard.clear(),
    }
}

/// public API. dynamic 우선, 실패 시 fallback. cache TTL 3600s.
///
/// `force=true` 면 cache 를 무시하고 항상 새로 가져온다 (성공 시 cache 갱신).
pub async fn discover_models(backend: &str, force: bool) -> anyhow::Result<DiscoveryResult> {
    // 1. cache lookup (force=false 일 때만)
    if !force {
        let guard = cache().read().await;
        if let Some(entry) = guard.get(backend) {
            let ttl_ok = entry.at.elapsed() < CACHE_TTL;
            let stamp_fresh = match &entry.binary_stamp {
                None => true,
                Some((path, stamp)) => std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map(|now| now == *stamp)
                    .unwrap_or(false),
            };
            if ttl_ok && stamp_fresh {
                return Ok(DiscoveryResult {
                    backend: backend.to_string(),
                    models: entry.models.clone(),
                    source: DiscoverySource::Cached,
                });
            }
        }
    }

    // 2. dynamic discovery
    let (discovered, binary_stamp): (Option<Vec<String>>, Option<(PathBuf, SystemTime)>) =
        match backend {
            "ollama" => (discover_ollama(None, None).await, None),
            "ollama_cloud" => (
                discover_ollama(Some("https://ollama.com"), env_cloud_api_key().as_deref()).await,
                None,
            ),
            "lmstudio" => (discover_lmstudio(None).await, None),
            "anthropic" => match discover_anthropic_with_stamp() {
                Some((m, p, t)) => (Some(m), Some((p, t))),
                None => (None, None),
            },
            "codex" => (discover_codex(), None),
            // OpenAI: discovery 생략 (API 호출 비용). 항상 fallback.
            "openai" => (None, None),
            // wiki claude alias backend ("haiku") — static, dynamic 무의미.
            "haiku" => (None, None),
            // wiki review: claude alias 라 dynamic 불필요.
            "claude" => (None, None),
            // disabled 는 모델 선택 자체가 의미 없음.
            "disabled" => (None, None),
            _ => (None, None),
        };

    if let Some(models) = discovered {
        if !models.is_empty() {
            let entry = CacheEntry {
                models: models.clone(),
                at: Instant::now(),
                binary_stamp,
            };
            let mut guard = cache().write().await;
            guard.insert(backend.to_string(), entry);
            return Ok(DiscoveryResult {
                backend: backend.to_string(),
                models,
                source: DiscoverySource::Dynamic,
            });
        }
    }

    // 3. fallback
    let models = fallback_models(backend);
    let entry = CacheEntry {
        models: models.clone(),
        at: Instant::now(),
        binary_stamp: None,
    };
    let mut guard = cache().write().await;
    guard.insert(backend.to_string(), entry);
    Ok(DiscoveryResult {
        backend: backend.to_string(),
        models,
        source: DiscoverySource::Fallback,
    })
}

// ─── Fallback registry ──────────────────────────────────────────────────────

/// backend 별 hardcoded fallback list. dynamic 실패 시에만 사용한다.
/// `llm::defaults` 의 상수를 우선 기반으로 하고, 실용 모델 몇 개 더 추가.
fn fallback_models(backend: &str) -> Vec<String> {
    match backend {
        "ollama" => vec![
            GRAPH_OLLAMA_DEFAULT.to_string(),
            "gemma3:12b".to_string(),
            "qwen3:8b".to_string(),
            "llama3.3:latest".to_string(),
        ],
        "ollama_cloud" => vec![
            GRAPH_OLLAMA_CLOUD_DEFAULT.to_string(),
            LOG_OLLAMA_CLOUD_DEFAULT.to_string(),
            WIKI_REVIEW_OLLAMA_CLOUD_DEFAULT.to_string(),
        ],
        "lmstudio" => vec![
            GRAPH_LMSTUDIO_DEFAULT.to_string(),
            "qwen2.5-coder-7b-instruct".to_string(),
        ],
        "anthropic" => vec![
            GRAPH_ANTHROPIC_DEFAULT.to_string(),
            "claude-sonnet-4-5".to_string(),
            "haiku".to_string(),
            "sonnet".to_string(),
            "opus".to_string(),
        ],
        "openai" => vec![
            "text-embedding-3-small".to_string(),
            "text-embedding-3-large".to_string(),
        ],
        "codex" => vec![
            WIKI_CODEX_DEFAULT.to_string(),
            "gpt-5.4-mini".to_string(),
            "gpt-5.3-codex".to_string(),
        ],
        // claude alias backend (wiki review 용): static 3개 — dynamic 불가.
        "claude" | "haiku" => {
            vec![
                "haiku".to_string(),
                "sonnet".to_string(),
                "opus".to_string(),
            ]
        }
        _ => Vec::new(),
    }
}

fn env_cloud_api_key() -> Option<String> {
    std::env::var("OLLAMA_CLOUD_API_KEY").ok()
}

// ─── Discovery 구현 ──────────────────────────────────────────────────────────

/// Ollama (local 또는 cloud): `GET {base}/api/tags`.
///
/// - `base` 기본값: `http://localhost:11434`. cloud 의 경우 `https://ollama.com`.
/// - `bearer` Some 이면 `Authorization: Bearer {token}` 헤더 추가.
async fn discover_ollama(base: Option<&str>, bearer: Option<&str>) -> Option<Vec<String>> {
    let base = base.unwrap_or("http://localhost:11434");
    let url = format!("{}/api/tags", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()?;
    let mut req = client.get(&url);
    if let Some(token) = bearer {
        req = req.bearer_auth(token);
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!(target: "secall::model_discovery", "ollama {} → {}", url, resp.status());
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let arr = body.get("models")?.as_array()?;
    let mut models: Vec<String> = arr
        .iter()
        .filter_map(|m| m.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();
    models.sort();
    models.dedup();
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

/// LM Studio: OpenAI-compatible `GET {base}/v1/models`.
async fn discover_lmstudio(base: Option<&str>) -> Option<Vec<String>> {
    let base = base.unwrap_or("http://localhost:1234");
    let url = format!("{}/v1/models", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!(target: "secall::model_discovery", "lmstudio {} → {}", url, resp.status());
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let data = body.get("data")?.as_array()?;
    let mut models: Vec<String> = data
        .iter()
        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    models.sort();
    models.dedup();
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

/// Codex: `~/.codex/models_cache.json` read.
fn discover_codex() -> Option<Vec<String>> {
    let path = dirs::home_dir()?.join(".codex").join("models_cache.json");
    if !path.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    let arr = data.get("models")?.as_array()?;
    let mut models = Vec::new();
    for m in arr {
        let slug = m.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        let vis = m.get("visibility").and_then(|v| v.as_str()).unwrap_or("");
        if !slug.is_empty() && vis != "hide" {
            models.push(slug.to_string());
        }
    }
    if models.is_empty() {
        None
    } else {
        Some(models)
    }
}

/// `which claude` 후 symlink resolve.
fn resolve_anthropic_binary() -> Option<PathBuf> {
    let (lookup, arg) = if cfg!(windows) {
        ("where", "claude")
    } else {
        ("which", "claude")
    };
    let output = std::process::Command::new(lookup).arg(arg).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let first = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    if first.is_empty() {
        return None;
    }
    std::fs::canonicalize(PathBuf::from(first)).ok()
}

/// 바이트 시퀀스에서 claude 모델 ID 문자열만 추출 (printable ASCII run 기준).
///
/// 정규식: `\bclaude-(opus|sonnet|haiku)-\d+(-\d+)?(-\d{8})?\b`.
pub(crate) fn extract_claude_model_ids(bytes: &[u8]) -> std::collections::BTreeSet<String> {
    use regex::Regex;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\bclaude-(?:opus|sonnet|haiku)-\d+(?:-\d+)?(?:-\d{8})?\b")
            .expect("claude model ID regex must compile (P65 model_discovery)")
    });
    let mut out = std::collections::BTreeSet::new();
    let is_printable = |b: u8| (32..127).contains(&b);
    let mut start = 0usize;
    for i in 0..bytes.len() {
        if !is_printable(bytes[i]) {
            if i > start {
                if let Ok(s) = std::str::from_utf8(&bytes[start..i]) {
                    for m in re.find_iter(s) {
                        out.insert(m.as_str().to_string());
                    }
                }
            }
            start = i + 1;
        }
    }
    if start < bytes.len() {
        if let Ok(s) = std::str::from_utf8(&bytes[start..]) {
            for m in re.find_iter(s) {
                out.insert(m.as_str().to_string());
            }
        }
    }
    out
}

/// claude binary 를 스캔하여 모델 ID + 바이너리 mtime 반환.
fn discover_anthropic_with_stamp() -> Option<(Vec<String>, PathBuf, SystemTime)> {
    let path = resolve_anthropic_binary()?;
    let meta = std::fs::metadata(&path).ok()?;
    let mtime = meta.modified().ok()?;
    // 300MB guard (tunaFlow 동일)
    const MAX_BIN_SIZE: u64 = 300 * 1024 * 1024;
    if meta.len() > MAX_BIN_SIZE {
        tracing::warn!(
            target: "secall::model_discovery",
            size = meta.len(),
            "claude binary too large, skipping scan"
        );
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    let ids = extract_claude_model_ids(&bytes);
    if ids.is_empty() {
        return None;
    }
    let mut models: Vec<String> = ids.into_iter().collect();
    // 정렬: family 우선 + 버전 내림차순. 단순화를 위해 알파벳 desc sort 사용
    // (claude-opus-4-7 > claude-opus-4-6 > claude-sonnet-* 와 같이 family
    // prefix 가 일정해서 일관된 순서가 나옴).
    models.sort_by(|a, b| b.cmp(a));
    // alias 도 prepend — CLI 가 항상 인식.
    for alias in ["haiku", "sonnet", "opus"] {
        if !models.iter().any(|m| m == alias) {
            models.insert(0, alias.to_string());
        }
    }
    Some((models, path, mtime))
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_backend_returns_empty_fallback() {
        let r = discover_models("nope_xyz", false).await.unwrap();
        assert_eq!(r.backend, "nope_xyz");
        assert_eq!(r.source, DiscoverySource::Fallback);
        assert!(r.models.is_empty());
    }

    #[tokio::test]
    async fn ollama_offline_falls_back_to_static_list() {
        // discover_ollama 가 localhost:11434 로 시도하지만 대부분의 CI/test
        // 환경에서는 ollama 가 떠 있지 않다 → fallback 경로를 탄다.
        // (만약 떠 있어도 dynamic 결과가 비어있지 않으면 그대로 통과)
        invalidate_cache(Some("ollama")).await;
        let r = discover_models("ollama", true).await.unwrap();
        assert_eq!(r.backend, "ollama");
        assert!(!r.models.is_empty(), "ollama models must never be empty");
        // GRAPH_OLLAMA_DEFAULT 가 fallback 또는 dynamic 결과에 들어있는지는
        // 환경 의존이므로 강제 assert 하지 않는다.
    }

    #[tokio::test]
    async fn cache_hit_returns_cached_source() {
        invalidate_cache(Some("openai")).await;
        // openai 는 discovery 없이 항상 fallback.
        let first = discover_models("openai", false).await.unwrap();
        assert_eq!(first.source, DiscoverySource::Fallback);
        let second = discover_models("openai", false).await.unwrap();
        assert_eq!(second.source, DiscoverySource::Cached);
        assert_eq!(first.models, second.models);
    }

    #[tokio::test]
    async fn force_bypasses_cache() {
        invalidate_cache(Some("openai")).await;
        let _ = discover_models("openai", false).await.unwrap();
        let forced = discover_models("openai", true).await.unwrap();
        // force=true 면 cache 무시 → fallback (openai 는 dynamic 없음).
        assert_eq!(forced.source, DiscoverySource::Fallback);
    }

    #[test]
    fn extract_claude_ids_basic() {
        let haystack = b"random\nclaude-opus-4-7 mid claude-sonnet-4-6 tail\nclaude-foo-1-0";
        let ids = extract_claude_model_ids(haystack);
        assert!(ids.contains("claude-opus-4-7"));
        assert!(ids.contains("claude-sonnet-4-6"));
        assert!(!ids.iter().any(|s| s.contains("foo")));
    }

    #[test]
    fn fallback_models_known_backends_non_empty() {
        for b in [
            "ollama",
            "ollama_cloud",
            "lmstudio",
            "anthropic",
            "openai",
            "codex",
            "claude",
        ] {
            let m = fallback_models(b);
            assert!(!m.is_empty(), "fallback for {} must not be empty", b);
        }
    }
}
