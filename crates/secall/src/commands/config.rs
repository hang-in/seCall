use std::path::PathBuf;

use anyhow::{Context, Result};
use secall_core::vault::Config;

pub fn run_show() -> Result<()> {
    let config = Config::load_or_default();
    let config_path = Config::config_path();

    println!("seCall Configuration");
    println!("====================");
    println!("Config file: {}", config_path.display());
    println!();

    println!("[vault]");
    println!("  path       = {}", config.vault.path.display());
    match &config.vault.git_remote {
        Some(remote) => println!("  git_remote = {}", remote),
        None => println!("  git_remote = (not set)"),
    }
    println!("  branch     = {}", config.vault.branch);
    println!();

    println!("[search]");
    println!("  tokenizer     = {}", config.search.tokenizer);
    println!("  default_limit = {}", config.search.default_limit);
    println!();

    println!("[embedding]");
    println!("  backend      = {}", config.embedding.backend);
    if let Some(url) = &config.embedding.ollama_url {
        println!("  ollama_url   = {}", url);
    }
    if let Some(model) = &config.embedding.ollama_model {
        println!("  ollama_model = {}", model);
    }
    if let Some(device) = &config.embedding.openvino_device {
        println!("  openvino_device = {}", device);
    }
    println!();

    println!("[openvino]");
    match &config.openvino.dir {
        Some(dir) => println!("  dir = {}", dir),
        None => println!("  dir = (auto-detect)"),
    }
    println!();

    println!("[output]");
    println!("  timezone = {}", config.output.timezone);

    Ok(())
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
                eprintln!("Warning: kiwi tokenizer is not supported on Windows. BM25 will fall back to lindera.");
            }
            config.search.tokenizer = value.to_string();
        }
        "search.default_limit" => {
            let n: usize = value
                .parse()
                .context("default_limit must be a positive integer")?;
            config.search.default_limit = n;
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
        "embedding.openvino_device" => {
            if !["NPU", "GPU", "CPU"].contains(&value) {
                anyhow::bail!("invalid openvino device: '{}'. Valid: NPU, GPU, CPU", value);
            }
            config.embedding.openvino_device = Some(value.to_string());
        }
        "openvino.dir" => {
            let path = std::path::PathBuf::from(shellexpand::tilde(value).to_string());
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
        _ => {
            anyhow::bail!(
                "unknown config key: '{}'\n\nAvailable keys:\n  \
                vault.path, vault.git_remote, vault.branch\n  \
                search.tokenizer, search.default_limit\n  \
                embedding.backend, embedding.ollama_url, embedding.ollama_model, embedding.openvino_device\n  \
                openvino.dir\n  \
                output.timezone",
                key
            );
        }
    }

    config.save()?;
    println!("✓ Set {} = {}", key, value);
    Ok(())
}

pub fn run_path() -> Result<()> {
    println!("{}", Config::config_path().display());
    Ok(())
}
