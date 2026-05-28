// secall-core library entrypoint
pub mod error;
pub mod graph;
pub mod hooks;
pub mod ingest;
pub mod jobs;
pub mod llm {
    pub mod defaults;
    pub mod model_discovery;
}
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

/// PATH (Windows 는 PATHEXT 포함) 에서 명령어의 실제 실행 파일 경로를 resolve 한다.
///
/// P87 (issue #92): Windows 에서 npm 으로 설치된 CLI (codex, claude 등) 는
/// `codex.cmd` 같은 배치 래퍼다. `std::process::Command::new("codex")` 는
/// PATHEXT 를 적용하지 않아 `.exe` 만 찾고 `.cmd` 는 "program not found" 로
/// 실패한다. `which` crate 는 PATHEXT 를 적용해 `.cmd`/`.bat`/`.exe` 를 모두
/// 탐색하므로 실제 경로를 얻을 수 있고, Rust 1.77+ 는 `.cmd`/`.bat` 확장자가
/// 포함된 경로를 `Command` 로 실행하면 cmd.exe 를 경유해 안전하게 실행한다.
///
/// resolve 실패 시 입력 문자열을 그대로 `PathBuf` 로 반환한다 (기존 동작 유지).
pub fn resolve_program(cmd: &str) -> std::path::PathBuf {
    which::which(cmd).unwrap_or_else(|_| std::path::PathBuf::from(cmd))
}

/// 크로스플랫폼 명령어 존재 확인.
///
/// P87 (issue #92): 외부 `where.exe` / `which` 프로세스 호출 대신 `which` crate
/// 사용 — Windows PATHEXT (`.cmd` 등) 를 적용하고, spawn 시 쓰는 [`resolve_program`]
/// 과 동일한 탐색 규칙이라 "존재하지만 spawn 실패" 불일치를 제거한다.
pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
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
