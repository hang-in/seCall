use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::types::{AgentKind, Role, Session, TokenUsage, Turn};
use super::SessionParser;

pub struct OpenCodeParser;

impl SessionParser for OpenCodeParser {
    fn can_parse(&self, _path: &Path) -> bool {
        false
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        parse_opencode_json(path).map_err(|e| crate::error::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e,
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::OpenCode
    }
}

// ─── Serde models ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenCodeExport {
    info: OpenCodeInfo,
    #[serde(default)]
    messages: Vec<OpenCodeMessage>,
}

#[derive(Deserialize)]
struct OpenCodeInfo {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    directory: Option<String>,
    time: OpenCodeTime,
}

#[derive(Deserialize)]
struct OpenCodeTime {
    created: u64,
    #[serde(default)]
    updated: Option<u64>,
}

#[derive(Deserialize)]
struct OpenCodeMessage {
    info: OpenCodeMessageInfo,
    #[serde(default)]
    parts: Vec<OpenCodePart>,
}

#[derive(Deserialize)]
struct OpenCodeMessageInfo {
    role: String,
    #[serde(default)]
    model: Option<OpenCodeModel>,
    #[serde(default)]
    time: Option<OpenCodeMsgTime>,
}

#[derive(Deserialize)]
struct OpenCodeModel {
    #[serde(rename = "modelID", default)]
    model_id: Option<String>,
}

#[derive(Deserialize)]
struct OpenCodeMsgTime {
    #[serde(default)]
    created: Option<u64>,
}

#[derive(Deserialize)]
struct OpenCodePart {
    #[serde(rename = "type")]
    part_type: String,
    #[serde(default)]
    text: Option<String>,
}

// ─── Parser ───────────────���───────────────────────────────���──────────────────

fn ms_to_datetime(ms: u64) -> Option<DateTime<Utc>> {
    let secs = (ms / 1000) as i64;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nsecs).single()
}

pub fn parse_opencode_json(path: &Path) -> Result<Session> {
    let raw = std::fs::read_to_string(path)?;
    let export: OpenCodeExport = serde_json::from_str(&raw)
        .map_err(|e| anyhow!("failed to parse opencode session {}: {e}", path.display()))?;

    let start_time = ms_to_datetime(export.info.time.created).unwrap_or_else(Utc::now);
    let end_time = export.info.time.updated.and_then(ms_to_datetime);

    let project = export.info.directory.as_deref().and_then(|d| {
        std::path::Path::new(d)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    });

    let mut session_model: Option<String> = None;
    let mut turns: Vec<Turn> = Vec::new();
    let mut turn_idx: u32 = 0;

    for msg in &export.messages {
        let role = match msg.info.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => Role::System,
        };

        if role == Role::Assistant && session_model.is_none() {
            session_model = msg
                .info
                .model
                .as_ref()
                .and_then(|m| m.model_id.clone());
        }

        let content: String = msg
            .parts
            .iter()
            .filter(|p| p.part_type == "text")
            .filter_map(|p| p.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");

        if content.is_empty() {
            continue;
        }

        let timestamp = msg
            .info
            .time
            .as_ref()
            .and_then(|t| t.created)
            .and_then(ms_to_datetime);

        turns.push(Turn {
            index: turn_idx,
            role,
            timestamp,
            content,
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        });
        turn_idx += 1;
    }

    if turns.is_empty() {
        return Err(anyhow!(
            "opencode session has no parseable turns: {}",
            path.display()
        ));
    }

    let _title = export.info.title;

    Ok(Session {
        id: export.info.id,
        agent: AgentKind::OpenCode,
        model: session_model,
        project,
        cwd: export.info.directory.map(std::path::PathBuf::from),
        git_branch: None,
        host: Some(gethostname::gethostname().to_string_lossy().to_string()),
        start_time,
        end_time,
        turns,
        total_tokens: TokenUsage::default(),
        session_type: "interactive".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_opencode_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::Builder::new()
            .prefix("ses_test-")
            .suffix(".json")
            .tempfile()
            .unwrap();
        write!(f, "{content}").unwrap();
        f
    }

    const BASIC_SESSION: &str = r#"{
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
    }"#;

    #[test]
    fn test_parse_opencode_session() {
        let f = make_opencode_file(BASIC_SESSION);
        let session = parse_opencode_json(f.path()).unwrap();
        assert_eq!(session.id, "ses_abc123");
        assert_eq!(session.agent, AgentKind::OpenCode);
        assert_eq!(session.project, Some("myapp".to_string()));
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[0].content, "Hello");
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert_eq!(session.turns[1].content, "Hi there!");
        assert!(!session.turns[1].content.contains("step-start"));
    }

    #[test]
    fn test_opencode_model_extraction() {
        let f = make_opencode_file(BASIC_SESSION);
        let session = parse_opencode_json(f.path()).unwrap();
        assert_eq!(session.model.as_deref(), Some("Qwen3.6-35B"));
    }

    #[test]
    fn test_opencode_timestamps() {
        let f = make_opencode_file(BASIC_SESSION);
        let session = parse_opencode_json(f.path()).unwrap();
        assert_eq!(session.start_time.date_naive().to_string(), "2026-04-25");
        assert!(session.end_time.is_some());
    }

    #[test]
    fn test_opencode_cwd() {
        let f = make_opencode_file(BASIC_SESSION);
        let session = parse_opencode_json(f.path()).unwrap();
        assert_eq!(
            session.cwd,
            Some(std::path::PathBuf::from("/Users/user/projects/myapp"))
        );
    }

    #[test]
    fn test_opencode_empty_turns() {
        let json = r#"{
            "info": {
                "id": "ses_empty",
                "time": { "created": 1777090810040 }
            },
            "messages": []
        }"#;
        let f = make_opencode_file(json);
        let result = parse_opencode_json(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_opencode_skip_tool_parts() {
        let json = r#"{
            "info": {
                "id": "ses_tools",
                "time": { "created": 1777090810040 }
            },
            "messages": [
                {
                    "info": { "role": "user", "time": { "created": 1777090810253 } },
                    "parts": [{ "type": "text", "text": "Do something" }]
                },
                {
                    "info": { "role": "assistant", "time": { "created": 1777090820000 } },
                    "parts": [
                        { "type": "tool-use", "toolName": "bash" },
                        { "type": "text", "text": "Done!" },
                        { "type": "tool-result", "result": "ok" }
                    ]
                }
            ]
        }"#;
        let f = make_opencode_file(json);
        let session = parse_opencode_json(f.path()).unwrap();
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[1].content, "Done!");
    }
}
