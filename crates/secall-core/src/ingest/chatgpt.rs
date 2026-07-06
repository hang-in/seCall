use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
use crate::ingest::SessionParser;

#[derive(Debug, Deserialize)]
struct GptConversation {
    conversation_id: String,
    title: Option<String>,
    create_time: Option<f64>,
    #[allow(dead_code)]
    update_time: Option<f64>,
    default_model_slug: Option<String>,
    mapping: HashMap<String, MappingNode>,
    current_node: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MappingNode {
    #[allow(dead_code)]
    id: String,
    message: Option<GptMessage>,
    parent: Option<String>,
    #[allow(dead_code)]
    children: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GptMessage {
    #[allow(dead_code)]
    id: String,
    author: GptAuthor,
    content: serde_json::Value,
    create_time: Option<f64>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GptAuthor {
    role: String,
}

fn read_conversations(path: &Path) -> crate::error::Result<Vec<GptConversation>> {
    let data = std::fs::read(path)?;

    let json_str = if data.starts_with(b"PK\x03\x04") {
        extract_conversations_from_zip(&data)?
    } else {
        String::from_utf8(data).map_err(|e| crate::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e.into(),
        })?
    };

    serde_json::from_str(&json_str).map_err(|e| crate::SecallError::Parse {
        path: path.to_string_lossy().into_owned(),
        source: e.into(),
    })
}

fn extract_conversations_from_zip(data: &[u8]) -> crate::error::Result<String> {
    let reader = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| crate::SecallError::Parse {
        path: "<zip>".to_string(),
        source: e.into(),
    })?;

    let mut file =
        archive
            .by_name("conversations.json")
            .map_err(|e| crate::SecallError::Parse {
                path: "<zip>/conversations.json".to_string(),
                source: anyhow::anyhow!("conversations.json not found in ZIP: {e}"),
            })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn linearize_mapping(conv: &GptConversation) -> Vec<&GptMessage> {
    let Some(mut node_id) = conv.current_node.clone() else {
        return Vec::new();
    };

    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();

    while visited.insert(node_id.clone()) {
        let Some(node) = conv.mapping.get(&node_id) else {
            break;
        };
        if let Some(message) = &node.message {
            chain.push(message);
        }
        let Some(parent) = &node.parent else {
            break;
        };
        node_id = parent.clone();
    }

    chain.reverse();
    chain
}

fn epoch_to_datetime(ts: Option<f64>) -> Option<DateTime<Utc>> {
    let value = ts?;
    if !value.is_finite() {
        return None;
    }

    let total_nanos = (value * 1_000_000_000.0).round() as i128;
    let secs = (total_nanos / 1_000_000_000) as i64;
    let nanos = (total_nanos % 1_000_000_000) as u32;
    DateTime::from_timestamp(secs, nanos)
}

fn sanitize_project_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    sanitized.trim().chars().take(50).collect()
}

fn extract_model_slug(conv: &GptConversation, chain: &[&GptMessage]) -> Option<String> {
    conv.default_model_slug.clone().or_else(|| {
        chain.iter().rev().find_map(|message| {
            message
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("model_slug"))
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    message
                        .metadata
                        .as_ref()
                        .and_then(|meta| meta.get("resolved_model_slug"))
                        .and_then(|v| v.as_str())
                        .map(ToOwned::to_owned)
                })
        })
    })
}

fn extract_parts(parts: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();

    for part in parts {
        if let Some(text) = part.as_str() {
            if !text.is_empty() {
                lines.push(text.to_string());
            }
        } else if !part.is_null() {
            lines.push("[attachment]".to_string());
        }
    }

    lines.join("\n")
}

fn extract_message_content(message: &GptMessage) -> (String, Option<String>) {
    let content_type = message
        .content
        .get("content_type")
        .and_then(|v| v.as_str())
        .unwrap_or("text");

    let text = match content_type {
        "text" | "multimodal_text" => message
            .content
            .get("parts")
            .and_then(|v| v.as_array())
            .map(|parts| extract_parts(parts))
            .unwrap_or_default(),
        // ChatGPT stores "code" payload under a `text` STRING field (no `parts`),
        // so route it to the text reader instead of the parts-only arm.
        "code" | "execution_output" => message
            .content
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        "reasoning_recap" => message
            .content
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        "thoughts" => message
            .content
            .get("thoughts")
            .and_then(|v| v.as_array())
            .map(|thoughts| {
                thoughts
                    .iter()
                    .filter_map(|t| t.get("content").and_then(|c| c.as_str()))
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        "tether_browsing_display" => message
            .content
            .get("result")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        "user_editable_context" => {
            let mut lines = Vec::new();
            if let Some(profile) = message
                .content
                .get("user_profile")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                lines.push(profile.to_string());
            }
            if let Some(instructions) = message
                .content
                .get("user_instructions")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                lines.push(instructions.to_string());
            }
            lines.join("\n")
        }
        _ => message
            .content
            .get("parts")
            .and_then(|v| v.as_array())
            .map(|parts| extract_parts(parts))
            .filter(|text| !text.is_empty())
            .or_else(|| {
                message
                    .content
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned)
            })
            .or_else(|| {
                message
                    .content
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_default(),
    };

    let thinking = match content_type {
        "thoughts" => {
            if text.is_empty() {
                Some(
                    message
                        .metadata
                        .as_ref()
                        .and_then(|meta| meta.get("reasoning_title"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("thinking")
                        .to_string(),
                )
            } else {
                Some(text.clone())
            }
        }
        "reasoning_recap" => Some(text.clone()),
        _ => None,
    };

    (text, thinking)
}

fn role_from_message(message: &GptMessage) -> Option<Role> {
    match message.author.role.as_str() {
        "user" => Some(Role::User),
        "assistant" => Some(Role::Assistant),
        "tool" => Some(Role::System),
        "system" => None,
        _ => Some(Role::System),
    }
}

fn conversation_to_session(conv: &GptConversation) -> crate::error::Result<Session> {
    let chain = linearize_mapping(conv);

    let mut turns = Vec::new();
    let mut end_time = None;

    for message in &chain {
        let Some(role) = role_from_message(message) else {
            continue;
        };

        let (content, thinking) = extract_message_content(message);
        if content.is_empty() && thinking.is_none() {
            continue;
        }

        let timestamp = epoch_to_datetime(message.create_time);
        if timestamp.is_some() {
            end_time = timestamp;
        }

        turns.push(Turn {
            index: turns.len() as u32,
            role,
            timestamp,
            content,
            actions: Vec::new(),
            tokens: None,
            thinking,
            is_sidechain: false,
        });
    }

    let project = conv
        .title
        .as_ref()
        .filter(|title| !title.is_empty())
        .map(|title| sanitize_project_name(title));

    Ok(Session {
        id: conv.conversation_id.clone(),
        agent: AgentKind::ChatGpt,
        model: extract_model_slug(conv, &chain),
        project,
        cwd: None,
        git_branch: None,
        host: Some(gethostname::gethostname().to_string_lossy().to_string()),
        start_time: epoch_to_datetime(conv.create_time)
            // When the conversation has no create_time, fall back to the earliest
            // available message timestamp (chain is chronological) before import time.
            .or_else(|| chain.iter().find_map(|m| epoch_to_datetime(m.create_time)))
            .unwrap_or_else(Utc::now),
        end_time,
        turns,
        total_tokens: TokenUsage::default(),
        session_type: "interactive".to_string(),
        archived: false,
        archived_at: None,
    })
}

pub struct ChatGptParser;

impl SessionParser for ChatGptParser {
    fn can_parse(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "zip" {
            return std::fs::read(path)
                .ok()
                .and_then(|bytes| extract_conversations_from_zip(&bytes).ok())
                .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
                .and_then(|value| value.as_array().and_then(|arr| arr.first().cloned()))
                .map(|first| first["mapping"].is_object() && first["conversation_id"].is_string())
                .unwrap_or(false);
        }

        if ext == "json" {
            return std::fs::read_to_string(path)
                .ok()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
                .and_then(|value| value.as_array().and_then(|arr| arr.first().cloned()))
                .map(|first| first["mapping"].is_object() && first["conversation_id"].is_string())
                .unwrap_or(false);
        }

        false
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        Err(crate::SecallError::UnsupportedFormat(format!(
            "ChatGPT export requires parse_all(): {}",
            path.display()
        )))
    }

    fn parse_all(&self, path: &Path) -> crate::error::Result<Vec<Session>> {
        let conversations = read_conversations(path)?;

        let mut sessions = Vec::new();
        for conv in &conversations {
            match conversation_to_session(conv) {
                Ok(session) => sessions.push(session),
                Err(error) => {
                    tracing::warn!(
                        conversation_id = %conv.conversation_id,
                        title = conv.title.as_deref().unwrap_or("(untitled)"),
                        error = %error,
                        "failed to parse chatgpt conversation, skipping"
                    );
                }
            }
        }

        Ok(sessions)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ChatGpt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_conversation_json() -> String {
        r#"[
          {
            "conversation_id": "conv-1",
            "title": "테스트 대화",
            "create_time": 1711234567.123,
            "default_model_slug": "gpt-4o",
            "current_node": "assistant-1",
            "mapping": {
              "system-1": {
                "id": "system-1",
                "message": {
                  "id": "system-1",
                  "author": { "role": "system" },
                  "content": { "content_type": "text", "parts": ["hidden"] },
                  "create_time": 1711234567.123,
                  "metadata": {}
                },
                "parent": null,
                "children": ["user-1"]
              },
              "user-1": {
                "id": "user-1",
                "message": {
                  "id": "user-1",
                  "author": { "role": "user" },
                  "content": { "content_type": "text", "parts": ["hello"] },
                  "create_time": 1711234568.0,
                  "metadata": {}
                },
                "parent": "system-1",
                "children": ["assistant-1"]
              },
              "assistant-1": {
                "id": "assistant-1",
                "message": {
                  "id": "assistant-1",
                  "author": { "role": "assistant" },
                  "content": { "content_type": "text", "parts": ["world"] },
                  "create_time": 1711234569.0,
                  "metadata": { "model_slug": "gpt-4o" }
                },
                "parent": "user-1",
                "children": []
              }
            }
          }
        ]"#
        .to_string()
    }

    fn sample_conversation() -> GptConversation {
        serde_json::from_str::<Vec<GptConversation>>(&sample_conversation_json())
            .unwrap()
            .remove(0)
    }

    #[test]
    fn test_linearize_simple_chain() {
        let conv = sample_conversation();
        let chain = linearize_mapping(&conv);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].author.role, "system");
        assert_eq!(chain[1].author.role, "user");
        assert_eq!(chain[2].author.role, "assistant");
    }

    #[test]
    fn test_linearize_with_regeneration() {
        let mut conv = sample_conversation();
        conv.current_node = Some("assistant-2".to_string());
        conv.mapping.insert(
            "assistant-2".to_string(),
            MappingNode {
                id: "assistant-2".to_string(),
                message: Some(GptMessage {
                    id: "assistant-2".to_string(),
                    author: GptAuthor {
                        role: "assistant".to_string(),
                    },
                    content: serde_json::json!({
                        "content_type": "text",
                        "parts": ["branch answer"]
                    }),
                    create_time: Some(1711234570.0),
                    metadata: Some(serde_json::json!({"model_slug": "gpt-4o"})),
                }),
                parent: Some("user-1".to_string()),
                children: Vec::new(),
            },
        );
        conv.mapping
            .get_mut("user-1")
            .unwrap()
            .children
            .push("assistant-2".to_string());

        let chain = linearize_mapping(&conv);
        assert_eq!(chain.last().unwrap().id, "assistant-2");
    }

    #[test]
    fn test_linearize_missing_current_node() {
        let mut conv = sample_conversation();
        conv.current_node = None;
        assert!(linearize_mapping(&conv).is_empty());
    }

    #[test]
    fn test_linearize_orphan_nodes() {
        let mut conv = sample_conversation();
        conv.current_node = Some("orphan".to_string());
        conv.mapping.insert(
            "orphan".to_string(),
            MappingNode {
                id: "orphan".to_string(),
                message: Some(GptMessage {
                    id: "orphan".to_string(),
                    author: GptAuthor {
                        role: "assistant".to_string(),
                    },
                    content: serde_json::json!({
                        "content_type": "text",
                        "parts": ["orphan reply"]
                    }),
                    create_time: Some(1711234571.0),
                    metadata: Some(serde_json::json!({})),
                }),
                parent: Some("missing-parent".to_string()),
                children: Vec::new(),
            },
        );

        let chain = linearize_mapping(&conv);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id, "orphan");
    }

    #[test]
    fn test_conversation_to_session_basic() {
        let conv = sample_conversation();
        let session = conversation_to_session(&conv).unwrap();

        assert_eq!(session.id, "conv-1");
        assert_eq!(session.agent, AgentKind::ChatGpt);
        assert_eq!(session.model.as_deref(), Some("gpt-4o"));
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[0].content, "hello");
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert_eq!(session.turns[1].content, "world");
    }

    #[test]
    fn test_epoch_to_datetime() {
        let dt = epoch_to_datetime(Some(1711234567.123)).unwrap();
        assert_eq!(dt.timestamp(), 1711234567);
        assert!((122..=123).contains(&dt.timestamp_subsec_millis()));
        assert!(epoch_to_datetime(None).is_none());
    }

    #[test]
    fn test_content_parts_text_extraction() {
        let message = GptMessage {
            id: "m1".to_string(),
            author: GptAuthor {
                role: "user".to_string(),
            },
            content: serde_json::json!({
                "content_type": "multimodal_text",
                "parts": ["hello", {"type":"image_asset_pointer"}, "world"]
            }),
            create_time: Some(1711234568.0),
            metadata: Some(serde_json::json!({})),
        };

        let (content, thinking) = extract_message_content(&message);
        assert_eq!(content, "hello\n[attachment]\nworld");
        assert!(thinking.is_none());
    }

    #[test]
    fn test_tool_role_handling() {
        let message = GptMessage {
            id: "tool-1".to_string(),
            author: GptAuthor {
                role: "tool".to_string(),
            },
            content: serde_json::json!({
                "content_type": "execution_output",
                "text": "tool output"
            }),
            create_time: Some(1711234568.0),
            metadata: Some(serde_json::json!({})),
        };

        assert_eq!(role_from_message(&message), Some(Role::System));
        let (content, _) = extract_message_content(&message);
        assert_eq!(content, "tool output");
    }

    #[test]
    fn test_thoughts_content_extraction() {
        let message = GptMessage {
            id: "thoughts-1".to_string(),
            author: GptAuthor {
                role: "assistant".to_string(),
            },
            content: serde_json::json!({
                "content_type": "thoughts",
                "thoughts": [
                    { "content": "first reasoning step", "type": "thought" },
                    { "content": "second reasoning step", "type": "thought" }
                ]
            }),
            create_time: Some(1711234568.0),
            metadata: Some(serde_json::json!({ "reasoning_title": "Thinking" })),
        };

        let (content, thinking) = extract_message_content(&message);
        assert_eq!(content, "first reasoning step\nsecond reasoning step");
        assert_eq!(
            thinking.as_deref(),
            Some("first reasoning step\nsecond reasoning step")
        );
    }

    #[test]
    fn test_thoughts_empty_falls_back_to_reasoning_title() {
        let message = GptMessage {
            id: "thoughts-2".to_string(),
            author: GptAuthor {
                role: "assistant".to_string(),
            },
            content: serde_json::json!({
                "content_type": "thoughts",
                "thoughts": []
            }),
            create_time: Some(1711234568.0),
            metadata: Some(serde_json::json!({ "reasoning_title": "Thinking" })),
        };

        let (content, thinking) = extract_message_content(&message);
        assert!(content.is_empty());
        assert_eq!(thinking.as_deref(), Some("Thinking"));
    }

    #[test]
    fn test_model_slug_extraction() {
        let conv = sample_conversation();
        let chain = linearize_mapping(&conv);
        assert_eq!(extract_model_slug(&conv, &chain).as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn test_parse_all_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conversations.json");
        std::fs::write(&path, sample_conversation_json()).unwrap();

        let parser = ChatGptParser;
        let sessions = parser.parse_all(&path).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "conv-1");
    }

    #[test]
    fn test_parse_all_from_zip() {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut buffer);
            writer
                .start_file::<_, ()>(
                    "conversations.json",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer
                .write_all(sample_conversation_json().as_bytes())
                .unwrap();
            writer.finish().unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chatgpt-export.zip");
        std::fs::write(&path, buffer.into_inner()).unwrap();

        let parser = ChatGptParser;
        let sessions = parser.parse_all(&path).unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].turns.len(), 2);
    }

    #[test]
    fn test_code_content_reads_text_field() {
        // ChatGPT "code" payload lives in a `text` STRING field (no `parts`).
        let message = GptMessage {
            id: "code-1".to_string(),
            author: GptAuthor {
                role: "assistant".to_string(),
            },
            content: serde_json::json!({
                "content_type": "code",
                "language": "python",
                "text": "print('hello')"
            }),
            create_time: Some(1711234568.0),
            metadata: Some(serde_json::json!({})),
        };

        let (content, thinking) = extract_message_content(&message);
        assert_eq!(content, "print('hello')");
        assert!(thinking.is_none());
    }

    #[test]
    fn test_start_time_falls_back_to_earliest_message() {
        let mut conv = sample_conversation();
        conv.create_time = None;
        let session = conversation_to_session(&conv).unwrap();
        // Earliest message in the chain (system-1) has create_time 1711234567.123.
        assert_eq!(session.start_time.timestamp(), 1711234567);
    }

    #[test]
    fn test_empty_conversations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conversations.json");
        std::fs::write(&path, "[]").unwrap();

        let parser = ChatGptParser;
        let sessions = parser.parse_all(&path).unwrap();
        assert!(sessions.is_empty());
    }
}
