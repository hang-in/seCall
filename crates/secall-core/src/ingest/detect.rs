use std::io::BufRead;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::{claude::ClaudeCodeParser, codex::CodexParser, gemini::GeminiParser, SessionParser};

pub fn detect_parser(path: &Path) -> Result<Box<dyn SessionParser>> {
    let path_str = path.to_string_lossy();

    // Path-based detection (fastest — check before content sniffing)
    if path_str.contains("/.claude/projects/") || path_str.contains("\\.claude\\projects\\") {
        return Ok(Box::new(ClaudeCodeParser));
    }
    if path_str.contains("/.codex/sessions/") || path_str.contains("\\.codex\\sessions\\") {
        return Ok(Box::new(CodexParser));
    }
    if (path_str.contains("/.gemini/") || path_str.contains("\\.gemini\\"))
        && path.extension().map(|e| e == "json").unwrap_or(false)
    {
        return Ok(Box::new(GeminiParser));
    }

    // Content sniffing: check first line of file
    if let Ok(file) = std::fs::File::open(path) {
        let mut reader = std::io::BufReader::new(file);
        let mut first_line = String::new();
        if reader.read_line(&mut first_line).is_ok() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&first_line) {
                // Claude Code: has "sessionId" + "type": "user"
                if v["sessionId"].is_string() && v["type"].as_str() == Some("user") {
                    return Ok(Box::new(ClaudeCodeParser));
                }
                // Codex: has "type" string + "message" object (adjacently tagged)
                if v["type"].is_string() && v["message"].is_object() {
                    return Ok(Box::new(CodexParser));
                }
            }

            // Gemini: single JSON file — check if first char is '{'
            // and it has "messages" array (need full parse, limited to < 100MB)
            if first_line.trim_start().starts_with('{') {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if metadata.len() < 100 * 1024 * 1024 {
                        if let Ok(raw) = std::fs::read_to_string(path) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                if v["messages"].is_array() && v["messages"][0]["parts"].is_array()
                                {
                                    return Ok(Box::new(GeminiParser));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!("unknown session format: {}", path.display()))
}

/// Find all Claude Code session files under the given base directory
pub fn find_claude_sessions(base: Option<&Path>) -> Result<Vec<std::path::PathBuf>> {
    let default_base;
    let base = match base {
        Some(b) => b,
        None => {
            default_base = dirs::home_dir()
                .ok_or_else(|| anyhow!("cannot determine home directory"))?
                .join(".claude")
                .join("projects");
            &default_base
        }
    };

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            paths.push(p.to_path_buf());
        }
    }
    Ok(paths)
}

/// Find all Codex session files under the given base directory
pub fn find_codex_sessions(base: Option<&Path>) -> Result<Vec<std::path::PathBuf>> {
    let default_base;
    let base = match base {
        Some(b) => b,
        None => {
            default_base = dirs::home_dir()
                .ok_or_else(|| anyhow!("cannot determine home directory"))?
                .join(".codex")
                .join("sessions");
            &default_base
        }
    };

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
            paths.push(p.to_path_buf());
        }
    }
    Ok(paths)
}

/// Find all Gemini CLI session files under the given base directory
pub fn find_gemini_sessions(base: Option<&Path>) -> Result<Vec<std::path::PathBuf>> {
    let default_base;
    let base = match base {
        Some(b) => b,
        None => {
            default_base = dirs::home_dir()
                .ok_or_else(|| anyhow!("cannot determine home directory"))?
                .join(".gemini")
                .join("tmp");
            &default_base
        }
    };

    if !base.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        // session-*.json files inside chats/ subdirectories
        if p.extension().map(|e| e == "json").unwrap_or(false) {
            let fname = p.file_name().unwrap_or_default().to_string_lossy();
            if fname.starts_with("session-") {
                paths.push(p.to_path_buf());
            }
        }
    }
    Ok(paths)
}

/// Find session files for a specific cwd (Claude Code only)
pub fn find_sessions_for_cwd(cwd: &Path) -> Result<Vec<std::path::PathBuf>> {
    let encoded = encode_cwd(cwd);
    let base = dirs::home_dir()
        .ok_or_else(|| anyhow!("cannot determine home directory"))?
        .join(".claude")
        .join("projects")
        .join(&encoded);
    find_claude_sessions(Some(&base))
}

/// Encode a path as Claude Code project directory name (/ → -)
pub fn encode_cwd(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace(['/', '\\'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_jsonl_file(dir: &std::path::Path, name: &str, lines: &[&str]) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        p
    }

    #[test]
    fn test_detect_claude_by_path() {
        let dir = tempfile::tempdir().unwrap();
        // Create a fake claude path
        let sub = dir.path().join(".claude").join("projects").join("proj");
        std::fs::create_dir_all(&sub).unwrap();
        let p = make_jsonl_file(
            &sub,
            "session.jsonl",
            &[r#"{"sessionId":"abc","type":"user","message":{"role":"user","content":[]}}"#],
        );
        let parser = detect_parser(&p).unwrap();
        assert_eq!(
            parser.agent_kind(),
            super::super::types::AgentKind::ClaudeCode
        );
    }

    #[test]
    fn test_detect_codex_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join(".codex").join("sessions").join("2026");
        std::fs::create_dir_all(&sub).unwrap();
        let p = make_jsonl_file(
            &sub,
            "rollout-abc.jsonl",
            &[r#"{"type":"user","message":{"role":"user","content":"hello"}}"#],
        );
        let parser = detect_parser(&p).unwrap();
        assert_eq!(parser.agent_kind(), super::super::types::AgentKind::Codex);
    }

    #[test]
    fn test_find_codex_sessions_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("nonexistent");
        let result = find_codex_sessions(Some(&base)).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_gemini_sessions_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("nonexistent");
        let result = find_gemini_sessions(Some(&base)).unwrap();
        assert!(result.is_empty());
    }
}
