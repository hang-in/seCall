use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use secall_core::command_exists;
use secall_core::llm::defaults::{
    GRAPH_ANTHROPIC_DEFAULT, GRAPH_GEMINI_DEFAULT, GRAPH_LMSTUDIO_DEFAULT,
};
use secall_core::vault::config::WikiBackendConfig;
use secall_core::vault::Config;

pub fn run_show() -> Result<()> {
    let config = Config::load_or_default();
    let config_path = Config::config_path();

    println!("seCall Configuration");
    println!("====================");
    println!("Config file: {}", config_path.display());
    println!();

    println!("Vault");
    println!("  path: {}", config.vault.path.display());
    println!(
        "  git_remote: {}",
        config.vault.git_remote.as_deref().unwrap_or("(not set)")
    );
    println!("  branch: {}", config.vault.branch);
    println!();

    print_llm_summary(&config);

    println!("Search");
    println!("  tokenizer: {}", config.search.tokenizer);
    println!("  default_limit: {}", config.search.default_limit);
    println!();

    println!("OpenVINO");
    println!(
        "  dir: {}",
        config.openvino.dir.as_deref().unwrap_or("(auto-detect)")
    );
    println!();

    println!("Output");
    println!("  timezone: {}", config.output.timezone);

    Ok(())
}

pub fn run_llm_show() -> Result<()> {
    let config = Config::load_or_default();
    print_llm_summary(&config);
    Ok(())
}

pub fn run_llm_where() -> Result<()> {
    println!("{}", Config::config_path().display());
    println!("LLM entry points:");
    println!("  CLI: secall config llm show");
    println!("  CLI: secall config llm set <key> <value>");
    println!("  REST: GET /api/config");
    println!("  REST: PATCH /api/config/{{wiki|graph|log|embedding}}");
    println!("  Web: /settings");
    Ok(())
}

#[derive(Clone, Copy)]
enum TestStatus {
    Ok,
    Fail,
    Skip,
}

struct TestOutcome {
    backend: String,
    status: TestStatus,
    detail: String,
}

impl TestStatus {
    fn as_str(self) -> &'static str {
        match self {
            TestStatus::Ok => "OK",
            TestStatus::Fail => "FAIL",
            TestStatus::Skip => "SKIP",
        }
    }
}

fn ok_outcome(backend: &str, detail: impl Into<String>) -> TestOutcome {
    TestOutcome {
        backend: backend.to_string(),
        status: TestStatus::Ok,
        detail: detail.into(),
    }
}

fn fail_outcome(backend: &str, detail: impl Into<String>) -> TestOutcome {
    TestOutcome {
        backend: backend.to_string(),
        status: TestStatus::Fail,
        detail: detail.into(),
    }
}

fn skip_outcome(backend: &str, detail: impl Into<String>) -> TestOutcome {
    TestOutcome {
        backend: backend.to_string(),
        status: TestStatus::Skip,
        detail: detail.into(),
    }
}

fn short_http_client() -> std::result::Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| format!("client build failed: {err}"))
}

pub async fn run_llm_test(backend: Option<String>, no_network: bool) -> Result<()> {
    let config = Config::load_or_default();
    let backends = match backend.as_deref() {
        None => vec!["claude", "codex", "haiku", "ollama", "lmstudio", "gemini"],
        Some(name @ ("claude" | "codex" | "haiku" | "ollama" | "lmstudio" | "gemini")) => {
            vec![name]
        }
        Some(other) => {
            anyhow::bail!(
                "unknown backend: {}. valid: claude/codex/haiku/ollama/lmstudio/gemini",
                other
            )
        }
    };

    let mut failed = false;
    for name in backends {
        let outcome = test_backend(&config, name, no_network).await;
        failed |= matches!(outcome.status, TestStatus::Fail);
        println!(
            "[{:<8}] {:<4} {}",
            outcome.backend,
            outcome.status.as_str(),
            outcome.detail
        );
    }

    if failed {
        std::process::exit(2);
    }

    Ok(())
}

fn print_llm_summary(config: &Config) {
    println!("Wiki");
    println!("  default_backend: {}", config.wiki.default_backend);
    println!(
        "  review_backend: {}",
        config
            .wiki
            .review_backend
            .as_deref()
            .unwrap_or("(inherits default_backend)")
    );
    println!(
        "  review_model: {}",
        config.wiki.review_model.as_deref().unwrap_or("sonnet")
    );
    if config.wiki.backends.is_empty() {
        println!("  backends: (not configured)");
    } else {
        println!("  backends:");
        for (name, backend) in &config.wiki.backends {
            print_backend_config(name, backend);
        }
    }
    println!();

    println!("Graph");
    println!("  semantic: {}", config.graph.semantic);
    println!("  semantic_backend: {}", config.graph.semantic_backend);
    println!(
        "  ollama_url: {}",
        config
            .graph
            .ollama_url
            .as_deref()
            .unwrap_or("http://localhost:11434")
    );
    println!(
        "  ollama_model: {}",
        config.graph.ollama_model.as_deref().unwrap_or("gemma4:e4b")
    );
    println!(
        "  anthropic_model: {}",
        config
            .graph
            .anthropic_model
            .as_deref()
            .unwrap_or("claude-haiku-4-5-20251001")
    );
    println!(
        "  gemini_model: {}",
        config
            .graph
            .gemini_model
            .as_deref()
            .unwrap_or("gemini-2.5-flash")
    );
    println!(
        "  gemini_api_key: {}",
        if config.graph.gemini_api_key.is_some() {
            "<masked>"
        } else {
            "<env: SECALL_GEMINI_API_KEY>"
        }
    );
    println!();

    println!("Log");
    println!(
        "  backend: {}",
        config
            .log
            .backend
            .as_deref()
            .unwrap_or(config.graph.semantic_backend.as_str())
    );
    println!(
        "  model: {}",
        config.log.model.as_deref().unwrap_or("(backend default)")
    );
    println!(
        "  api_url: {}",
        config.log.api_url.as_deref().unwrap_or("(backend default)")
    );
    println!(
        "  max_tokens: {}",
        config
            .log
            .max_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "(backend default)".to_string())
    );
    println!();

    println!("Embedding");
    println!("  backend: {}", config.embedding.backend);
    println!(
        "  ollama_url: {}",
        config
            .embedding
            .ollama_url
            .as_deref()
            .unwrap_or("(not set)")
    );
    println!(
        "  ollama_model: {}",
        config
            .embedding
            .ollama_model
            .as_deref()
            .unwrap_or("(not set)")
    );
    println!(
        "  openai_model: {}",
        config
            .embedding
            .openai_model
            .as_deref()
            .unwrap_or("(not set)")
    );
    println!(
        "  openvino_device: {}",
        config
            .embedding
            .openvino_device
            .as_deref()
            .unwrap_or("(not set)")
    );
    println!();

    println!("Environment indicators");
    for (key, present) in [
        (
            "ANTHROPIC_API_KEY",
            std::env::var("ANTHROPIC_API_KEY").is_ok(),
        ),
        (
            "SECALL_GEMINI_API_KEY",
            std::env::var("SECALL_GEMINI_API_KEY").is_ok(),
        ),
        ("OPENAI_API_KEY", std::env::var("OPENAI_API_KEY").is_ok()),
    ] {
        println!("  {}: {}", key, if present { "set" } else { "not set" });
    }
}

async fn test_backend(config: &Config, backend: &str, no_network: bool) -> TestOutcome {
    match backend {
        "claude" => test_cli_backend("claude", no_network).await,
        "codex" => test_cli_backend("codex", no_network).await,
        "haiku" => test_haiku_backend(config, no_network).await,
        "ollama" => test_ollama_backend(config, no_network).await,
        "lmstudio" => test_lmstudio_backend(config, no_network).await,
        "gemini" => test_gemini_backend(config, no_network).await,
        _ => fail_outcome(backend, "unsupported backend"),
    }
}

async fn test_cli_backend(bin: &'static str, no_network: bool) -> TestOutcome {
    if !command_exists(bin) {
        return fail_outcome(bin, "not installed (PATH lookup failed)");
    }

    let path = resolve_command_path(bin).unwrap_or_else(|| "(in PATH)".to_string());
    if no_network {
        return ok_outcome(bin, path);
    }

    match run_version_command(bin).await {
        Ok(version) => ok_outcome(bin, format!("{} ({})", path, version)),
        Err(err) => fail_outcome(bin, err),
    }
}

async fn test_haiku_backend(config: &Config, no_network: bool) -> TestOutcome {
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(value) if !value.is_empty() => value,
        _ => return fail_outcome("haiku", "ANTHROPIC_API_KEY not set"),
    };

    if no_network {
        return ok_outcome("haiku", "ANTHROPIC_API_KEY set");
    }

    let model = config
        .graph
        .anthropic_model
        .as_deref()
        .unwrap_or(GRAPH_ANTHROPIC_DEFAULT);
    let client = match short_http_client() {
        Ok(client) => client,
        Err(err) => return fail_outcome("haiku", err),
    };

    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 1,
        "system": "Reply with hi",
        "messages": [{"role": "user", "content": "hi"}]
    });

    match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            ok_outcome("haiku", format!("{} 1-token call {}", model, resp.status()))
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            fail_outcome(
                "haiku",
                format!("{} {}", status, truncate_for_display(&body)),
            )
        }
        Err(err) => fail_outcome("haiku", err.to_string()),
    }
}

async fn test_ollama_backend(config: &Config, no_network: bool) -> TestOutcome {
    let url = config
        .graph
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    if no_network {
        return ok_outcome("ollama", url);
    }

    let endpoint = format!("{}/api/tags", url.trim_end_matches('/'));
    let client = match short_http_client() {
        Ok(client) => client,
        Err(err) => return fail_outcome("ollama", err),
    };

    match client.get(&endpoint).send().await {
        Ok(resp) if resp.status().is_success() => {
            let status = resp.status();
            let body: serde_json::Value = match resp.json().await {
                Ok(value) => value,
                Err(err) => {
                    return fail_outcome("ollama", format!("invalid JSON response: {err}"));
                }
            };
            let model = config.graph.ollama_model.as_deref().unwrap_or("(default)");
            let has_models = body.get("models").and_then(|v| v.as_array()).is_some();
            if has_models {
                ok_outcome("ollama", format!("{} ({}, model {})", url, status, model))
            } else {
                fail_outcome("ollama", format!("{} missing models array", url))
            }
        }
        Ok(resp) => fail_outcome("ollama", format!("{} {}", resp.status(), url)),
        Err(err) => fail_outcome("ollama", err.to_string()),
    }
}

async fn test_lmstudio_backend(config: &Config, no_network: bool) -> TestOutcome {
    let Some(url) = lmstudio_url(config) else {
        return skip_outcome("lmstudio", "api_url not configured");
    };

    if no_network {
        return ok_outcome("lmstudio", url);
    }

    let endpoint = format!("{}/v1/models", url.trim_end_matches('/'));
    let client = match short_http_client() {
        Ok(client) => client,
        Err(err) => return fail_outcome("lmstudio", err),
    };

    match client.get(&endpoint).send().await {
        Ok(resp) if resp.status().is_success() => ok_outcome(
            "lmstudio",
            format!(
                "{} ({}, model {})",
                url,
                resp.status(),
                config
                    .graph
                    .ollama_model
                    .as_deref()
                    .unwrap_or(GRAPH_LMSTUDIO_DEFAULT)
            ),
        ),
        Ok(resp) => fail_outcome("lmstudio", format!("{} {}", resp.status(), url)),
        Err(err) => fail_outcome("lmstudio", err.to_string()),
    }
}

async fn test_gemini_backend(config: &Config, no_network: bool) -> TestOutcome {
    let api_key = match config
        .graph
        .gemini_api_key
        .clone()
        .or_else(|| std::env::var("SECALL_GEMINI_API_KEY").ok())
    {
        Some(value) if !value.is_empty() => value,
        _ => return fail_outcome("gemini", "SECALL_GEMINI_API_KEY not set"),
    };

    if no_network {
        let _ = api_key;
        return ok_outcome("gemini", "SECALL_GEMINI_API_KEY set");
    }

    let model = config
        .graph
        .gemini_model
        .as_deref()
        .unwrap_or(GRAPH_GEMINI_DEFAULT);
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );
    let payload = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": "hi"}]
        }],
        "generationConfig": {"maxOutputTokens": 1}
    });
    let client = match short_http_client() {
        Ok(client) => client,
        Err(err) => return fail_outcome("gemini", err),
    };

    match client.post(url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => ok_outcome(
            "gemini",
            format!("{} 1-token call {}", model, resp.status()),
        ),
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            fail_outcome(
                "gemini",
                format!("{} {}", status, truncate_for_display(&body)),
            )
        }
        Err(err) => fail_outcome("gemini", err.to_string()),
    }
}

fn lmstudio_url(config: &Config) -> Option<String> {
    config
        .graph
        .ollama_url
        .clone()
        .or_else(|| {
            if config.graph.semantic_backend == "lmstudio" {
                Some("http://localhost:1234".to_string())
            } else {
                None
            }
        })
        .or_else(|| {
            if config.log.backend.as_deref() == Some("lmstudio") {
                config.log.api_url.clone()
            } else {
                None
            }
        })
        .or_else(|| {
            if config.wiki.default_backend == "lmstudio" {
                config
                    .wiki
                    .backends
                    .get("lmstudio")
                    .and_then(|cfg| cfg.api_url.clone())
            } else {
                None
            }
        })
        .or_else(|| {
            config
                .wiki
                .backends
                .get("lmstudio")
                .and_then(|cfg| cfg.api_url.clone())
        })
}

fn resolve_command_path(cmd: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let output = Command::new("where.exe").arg(cmd).output().ok()?;
    #[cfg(not(target_os = "windows"))]
    let output = Command::new("which").arg(cmd).output().ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .and_then(|stdout| stdout.lines().next().map(|line| line.trim().to_string()))
}

async fn run_version_command(cmd: &str) -> std::result::Result<String, String> {
    let mut child = tokio::process::Command::new(cmd);
    child.arg("--version");
    child.kill_on_drop(true);

    let output = match tokio::time::timeout(Duration::from_secs(5), child.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => return Err(err.to_string()),
        Err(_) => return Err("timed out after 5s".to_string()),
    };

    if !output.status.success() {
        return Err(truncate_for_display(&String::from_utf8_lossy(
            &output.stderr,
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(truncate_for_display(stdout.trim()))
}

fn truncate_for_display(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 200 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..200])
    }
}

fn print_backend_config(name: &str, backend: &WikiBackendConfig) {
    println!(
        "    {}: model={} api_url={} max_tokens={}",
        name,
        backend.model.as_deref().unwrap_or("(default)"),
        backend.api_url.as_deref().unwrap_or("(default)"),
        backend.max_tokens
    );
}

pub fn run_set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_or_default();

    match key {
        "vault.path" => {
            let path = PathBuf::from(shellexpand::tilde(value).to_string());
            if !path.exists() {
                eprintln!("Warning: directory does not exist: {}", path.display());
            }
            config.vault.path = path;
        }
        "vault.git_remote" => {
            config.vault.git_remote = Some(value.to_string());
        }
        "vault.branch" => {
            config.vault.branch = value.to_string();
        }
        "search.tokenizer" => {
            if !["lindera", "kiwi"].contains(&value) {
                anyhow::bail!("invalid tokenizer: '{}'. Valid: lindera, kiwi", value);
            }
            #[cfg(target_os = "windows")]
            if value == "kiwi" {
                eprintln!(
                    "Warning: kiwi tokenizer is not supported on Windows. BM25 will fall back to lindera."
                );
            }
            config.search.tokenizer = value.to_string();
        }
        "search.default_limit" => {
            config.search.default_limit = value
                .parse()
                .context("default_limit must be a positive integer")?;
        }
        "wiki.default_backend" => {
            config.wiki.default_backend = value.to_string();
        }
        "wiki.review_backend" => {
            config.wiki.review_backend = Some(value.to_string());
        }
        "wiki.review_model" => {
            config.wiki.review_model = Some(value.to_string());
        }
        "graph.semantic_backend" => {
            config.graph.semantic_backend = value.to_string();
        }
        "graph.ollama_url" => {
            config.graph.ollama_url = Some(value.to_string());
        }
        "graph.ollama_model" => {
            config.graph.ollama_model = Some(value.to_string());
        }
        "graph.anthropic_model" => {
            config.graph.anthropic_model = Some(value.to_string());
        }
        "graph.gemini_model" => {
            config.graph.gemini_model = Some(value.to_string());
        }
        "graph.gemini_api_key" => {
            config.graph.gemini_api_key = Some(value.to_string());
        }
        "log.backend" => {
            config.log.backend = Some(value.to_string());
        }
        "log.model" => {
            config.log.model = Some(value.to_string());
        }
        "log.api_url" => {
            config.log.api_url = Some(value.to_string());
        }
        "log.max_tokens" => {
            config.log.max_tokens = Some(
                value
                    .parse()
                    .context("log.max_tokens must be a positive integer")?,
            );
        }
        "embedding.backend" => {
            if !["ollama", "ort", "openai", "openvino", "none"].contains(&value) {
                anyhow::bail!(
                    "invalid backend: '{}'. Valid: ollama, ort, openai, openvino, none",
                    value
                );
            }
            config.embedding.backend = value.to_string();
        }
        "embedding.ollama_url" => {
            config.embedding.ollama_url = Some(value.to_string());
        }
        "embedding.ollama_model" => {
            config.embedding.ollama_model = Some(value.to_string());
        }
        "embedding.openai_model" => {
            config.embedding.openai_model = Some(value.to_string());
        }
        "embedding.openvino_device" => {
            if !["NPU", "GPU", "CPU"].contains(&value) {
                anyhow::bail!("invalid openvino device: '{}'. Valid: NPU, GPU, CPU", value);
            }
            config.embedding.openvino_device = Some(value.to_string());
        }
        "openvino.dir" => {
            let path = PathBuf::from(shellexpand::tilde(value).to_string());
            if !path.exists() {
                eprintln!("Warning: directory does not exist: {}", path.display());
            }
            config.openvino.dir = Some(path.to_string_lossy().to_string());
        }
        "output.timezone" => {
            value.parse::<chrono_tz::Tz>().map_err(|_| {
                anyhow::anyhow!(
                    "invalid timezone: '{}'. Use IANA format (e.g. Asia/Seoul)",
                    value
                )
            })?;
            config.output.timezone = value.to_string();
        }
        _ if key.starts_with("wiki.backends.") => {
            set_wiki_backend_key(&mut config, key, value)?;
        }
        _ => anyhow::bail!("unknown config key: '{}'", key),
    }

    config.save()?;
    println!("✓ Set {} = {}", key, value);
    Ok(())
}

fn set_wiki_backend_key(config: &mut Config, key: &str, value: &str) -> Result<()> {
    let rest = key
        .strip_prefix("wiki.backends.")
        .ok_or_else(|| anyhow::anyhow!("invalid wiki backend key: {}", key))?;
    let (backend_name, field) = rest
        .rsplit_once('.')
        .ok_or_else(|| anyhow::anyhow!("invalid wiki backend key: {}", key))?;
    let entry = config
        .wiki
        .backends
        .entry(backend_name.to_string())
        .or_default();

    match field {
        "api_url" => entry.api_url = Some(value.to_string()),
        "model" => entry.model = Some(value.to_string()),
        "max_tokens" => {
            entry.max_tokens = value
                .parse()
                .context("wiki backend max_tokens must be a positive integer")?
        }
        _ => anyhow::bail!("unknown wiki backend field: {}", field),
    }

    Ok(())
}

pub fn run_path(copy: bool) -> Result<()> {
    let path = Config::config_path();
    println!("{}", path.display());
    if copy {
        copy_to_clipboard(&path)?;
    }
    Ok(())
}

fn copy_to_clipboard(path: &std::path::Path) -> Result<()> {
    let path_text = path.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &["pbcopy"];
    #[cfg(not(target_os = "macos"))]
    let candidates: &[&str] = &["xclip", "xsel"];

    for cmd in candidates {
        let child = match *cmd {
            "xclip" => Command::new(cmd)
                .args(["-selection", "clipboard"])
                .stdin(Stdio::piped())
                .spawn(),
            "xsel" => Command::new(cmd)
                .args(["--clipboard", "--input"])
                .stdin(Stdio::piped())
                .spawn(),
            _ => Command::new(cmd).stdin(Stdio::piped()).spawn(),
        };

        if let Ok(mut child) = child {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write as _;
                stdin.write_all(path_text.as_bytes())?;
            }
            let status = child.wait()?;
            if status.success() {
                return Ok(());
            }
        }
    }

    anyhow::bail!(
        "clipboard command not available (tried: {})",
        candidates.join(", ")
    );
}
