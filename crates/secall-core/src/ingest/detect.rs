use std::io::BufRead;
use std::path::Path;

use anyhow::{anyhow, Result};

use super::{
    chatgpt::ChatGptParser, claude::ClaudeCodeParser, claude_ai::ClaudeAiParser,
    codex::CodexParser, gemini::GeminiParser, gemini_web::GeminiWebParser,
    opencode::OpenCodeParser, SessionParser,
};

pub fn detect_parser(path: &Path) -> Result<Box<dyn SessionParser>> {
    let path_str = path.to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // Path-based detection (fastest — check before content sniffing)
    if path_str.contains("/.claude/projects/") || path_str.contains("\\.claude\\projects\\") {
        return Ok(Box::new(ClaudeCodeParser));
    }
    if path_str.contains("/.codex/sessions/") || path_str.contains("\\.codex\\sessions\\") {
        return Ok(Box::new(CodexParser));
    }
    if (path_str.contains("/.gemini/") || path_str.contains("\\.gemini\\")) && ext == "json" {
        return Ok(Box::new(GeminiParser));
    }

    // claude.ai / ChatGPT export: ZIP 파일 (.zip 확장자)
    if ext == "zip" {
        if let Ok(data) = std::fs::read(path) {
            if data.starts_with(b"PK\x03\x04") {
                // GeminiWeb: 아카이브 내 첫 .json 파일의 projectHash 확인
                if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&data)) {
                    let names: Vec<String> = archive.file_names().map(|s| s.to_owned()).collect();
                    if let Some(name) = names.iter().find(|n| n.ends_with(".json")) {
                        if let Ok(mut f) = archive.by_name(name) {
                            let mut raw = String::new();
                            if std::io::Read::read_to_string(&mut f, &mut raw).is_ok() {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                    if v["projectHash"].as_str() == Some("gemini-web") {
                                        return Ok(Box::new(GeminiWebParser));
                                    }
                                }
                            }
                        }
                    }
                }

                if let Ok(file) = std::fs::File::open(path) {
                    if let Ok(mut archive) = zip::ZipArchive::new(file) {
                        if let Ok(mut conversations) = archive.by_name("conversations.json") {
                            let mut raw = String::new();
                            if std::io::Read::read_to_string(&mut conversations, &mut raw).is_ok() {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                    if let Some(first) = v.as_array().and_then(|arr| arr.first()) {
                                        if first["chat_messages"].is_array()
                                            && first["uuid"].is_string()
                                        {
                                            return Ok(Box::new(ClaudeAiParser));
                                        }
                                        if first["mapping"].is_object()
                                            && first["conversation_id"].is_string()
                                        {
                                            return Ok(Box::new(ChatGptParser));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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

            // JSON object: full parse for opencode / Gemini (limited to < 100MB)
            if first_line.trim_start().starts_with('{') {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if metadata.len() < 100 * 1024 * 1024 {
                        if let Ok(raw) = std::fs::read_to_string(path) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                                // opencode: top-level "info" with "id" + "messages" array
                                if v["info"]["id"].is_string() && v["messages"].is_array() {
                                    return Ok(Box::new(OpenCodeParser));
                                }
                                // Gemini: "messages" array with parts sub-array
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

    // claude.ai / ChatGPT export: conversations.json (JSON array)
    if ext == "json" {
        if let Ok(data) = std::fs::read_to_string(path) {
            if data.trim_start().starts_with('[') {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(arr) = v.as_array() {
                        if let Some(first) = arr.first() {
                            if first["chat_messages"].is_array() && first["uuid"].is_string() {
                                return Ok(Box::new(ClaudeAiParser));
                            }
                            if first["mapping"].is_object() && first["conversation_id"].is_string()
                            {
                                return Ok(Box::new(ChatGptParser));
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
    fn test_detect_chatgpt_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conversations.json");
        std::fs::write(
            &path,
            r#"[
              {
                "conversation_id": "conv-1",
                "title": "chatgpt",
                "create_time": 1711234567.123,
                "mapping": {},
                "current_node": null
              }
            ]"#,
        )
        .unwrap();

        let parser = detect_parser(&path).unwrap();
        assert_eq!(parser.agent_kind(), super::super::types::AgentKind::ChatGpt);
    }

    #[test]
    fn test_detect_chatgpt_vs_claude_ai() {
        let dir = tempfile::tempdir().unwrap();

        let claude_path = dir.path().join("claude-conversations.json");
        std::fs::write(
            &claude_path,
            r#"[
              {
                "uuid": "conv-1",
                "chat_messages": []
              }
            ]"#,
        )
        .unwrap();
        let claude_parser = detect_parser(&claude_path).unwrap();
        assert_eq!(
            claude_parser.agent_kind(),
            super::super::types::AgentKind::ClaudeAi
        );

        let chatgpt_path = dir.path().join("chatgpt-conversations.json");
        std::fs::write(
            &chatgpt_path,
            r#"[
              {
                "conversation_id": "conv-2",
                "mapping": {},
                "current_node": null
              }
            ]"#,
        )
        .unwrap();
        let chatgpt_parser = detect_parser(&chatgpt_path).unwrap();
        assert_eq!(
            chatgpt_parser.agent_kind(),
            super::super::types::AgentKind::ChatGpt
        );
    }

    #[test]
    fn test_find_gemini_sessions_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("nonexistent");
        let result = find_gemini_sessions(Some(&base)).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_claude_ai_json() {
        let dir = tempfile::tempdir().unwrap();
        let json_path = dir.path().join("conversations.json");
        std::fs::write(
            &json_path,
            r#"[{"uuid":"test","name":"","created_at":"2026-01-01T00:00:00Z","chat_messages":[{"uuid":"m1","text":"hi","content":[],"sender":"human","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","attachments":[],"files":[]}]}]"#,
        )
        .unwrap();
        let parser = detect_parser(&json_path).unwrap();
        assert_eq!(
            parser.agent_kind(),
            super::super::types::AgentKind::ClaudeAi
        );
    }

    #[test]
    fn test_detect_opencode_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ses_abc123.json");
        std::fs::write(
            &path,
            r#"{
              "info": {
                "id": "ses_abc123",
                "slug": "test-session",
                "projectID": "proj1",
                "directory": "/Users/user/projects/myapp",
                "title": "Test session",
                "version": "1.14.24",
                "time": { "created": 1777090810040, "updated": 1777091142209 }
              },
              "messages": [
                {
                  "info": {
                    "role": "user",
                    "id": "msg_001",
                    "sessionID": "ses_abc123",
                    "time": { "created": 1777090810253 }
                  },
                  "parts": [
                    { "type": "text", "text": "Hello", "id": "prt_001", "sessionID": "ses_abc123", "messageID": "msg_001" }
                  ]
                },
                {
                  "info": {
                    "role": "assistant",
                    "id": "msg_002",
                    "sessionID": "ses_abc123",
                    "model": { "providerID": "llama", "modelID": "Qwen3.6-35B" },
                    "time": { "created": 1777090820000 }
                  },
                  "parts": [
                    { "type": "step-start", "snapshot": {} },
                    { "type": "text", "text": "Hi there!", "id": "prt_002", "sessionID": "ses_abc123", "messageID": "msg_002" }
                  ]
                }
              ]
            }"#,
        )
        .unwrap();

        let parser = detect_parser(&path).unwrap();
        assert_eq!(
            parser.agent_kind(),
            super::super::types::AgentKind::OpenCode
        );
    }
}
