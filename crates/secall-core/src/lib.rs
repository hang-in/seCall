// secall-core library entrypoint
pub mod error;
pub mod graph;
pub mod hooks;
pub mod ingest;
pub mod jobs;
pub mod mcp;
pub mod search;
pub mod store;
pub mod vault;
#[cfg(feature = "web-ui")]
pub mod web;
pub mod wiki;

pub use error::{Result, SecallError};

/// 간단한 HTTP POST JSON 헬퍼 (Ollama 모델 언로드 등 내부용)
pub async fn http_post_json(url: &str, body: &serde_json::Value) -> anyhow::Result<()> {
    reqwest::Client::new().post(url).json(body).send().await?;
    Ok(())
}

/// 크로스플랫폼 명령어 존재 확인
pub fn command_exists(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    let check = std::process::Command::new("where.exe")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    #[cfg(not(target_os = "windows"))]
    let check = std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_exists_known() {
        // git은 개발 환경에서 반드시 존재
        assert!(command_exists("git"));
    }

    #[test]
    fn test_command_exists_unknown() {
        assert!(!command_exists("__nonexistent_command_xyz_12345__"));
    }
}
