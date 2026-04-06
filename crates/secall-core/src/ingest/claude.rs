use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::DateTime;
use serde_json::Value;

use super::types::{Action, AgentKind, Role, Session, TokenUsage, Turn};
use super::SessionParser;

const TOOL_OUTPUT_MAX_CHARS: usize = 1000;

pub struct ClaudeCodeParser;

impl SessionParser for ClaudeCodeParser {
    fn can_parse(&self, path: &Path) -> bool {
        // Match ~/.claude/projects/**/*.jsonl pattern
        let path_str = path.to_string_lossy();
        (path_str.contains("/.claude/projects/") || path_str.contains("\\.claude\\projects\\"))
            && path.extension().map(|e| e == "jsonl").unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        parse_claude_jsonl(path).map_err(|e| crate::error::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e,
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ClaudeCode
    }
}

pub fn parse_claude_jsonl(path: &Path) -> Result<Session> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut session_id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut cwd: Option<std::path::PathBuf> = None;
    let mut git_branch: Option<String> = None;
    let mut first_timestamp: Option<DateTime<chrono::Utc>> = None;
    let mut last_timestamp: Option<DateTime<chrono::Utc>> = None;
    let mut turns: Vec<Turn> = Vec::new();
    let mut total_tokens = TokenUsage::default();

    // Pending tool_use entries keyed by tool_use_id waiting for tool_result
    let mut pending_tool_uses: HashMap<String, usize> = HashMap::new(); // tool_use_id -> action index in last assistant turn

    let mut line_count = 0;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        line_count += 1;

        let value: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "JSON parse error (skipping line)");
                continue;
            }
        };

        let msg_type = match value["type"].as_str() {
            Some(t) => t,
            None => continue,
        };

        // Extract timestamp
        let ts = value["timestamp"]
            .as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        if let Some(t) = ts {
            if first_timestamp.is_none() {
                first_timestamp = Some(t);
            }
            last_timestamp = Some(t);
        }

        match msg_type {
            "user" => {
                // Extract session metadata from first user message
                if session_id.is_none() {
                    session_id = value["sessionId"].as_str().map(String::from);
                }
                if cwd.is_none() {
                    cwd = value["cwd"].as_str().map(std::path::PathBuf::from);
                }
                if git_branch.is_none() {
                    git_branch = value["gitBranch"].as_str().map(String::from);
                }

                let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);
                let message = &value["message"];
                let content_val = &message["content"];

                // Check if this is a tool_result message
                if content_val.is_array() {
                    let items = content_val.as_array().unwrap();
                    let has_tool_result = items
                        .iter()
                        .any(|item| item["type"].as_str() == Some("tool_result"));

                    if has_tool_result {
                        // Attach tool results to the last assistant turn
                        for item in items {
                            if item["type"].as_str() == Some("tool_result") {
                                let tool_use_id =
                                    item["tool_use_id"].as_str().unwrap_or("").to_string();
                                let output = extract_tool_result_content(&item["content"]);
                                let truncated = truncate_str(&output, TOOL_OUTPUT_MAX_CHARS);

                                // Find the corresponding action in the last assistant turn
                                if let Some(&action_idx) = pending_tool_uses.get(&tool_use_id) {
                                    if let Some(Action::ToolUse { output_summary, .. }) = turns
                                        .last_mut()
                                        .and_then(|turn| turn.actions.get_mut(action_idx))
                                    {
                                        *output_summary = truncated;
                                    }
                                }
                            }
                        }
                        pending_tool_uses.clear();
                        continue;
                    }
                }

                // Regular user message
                let text = extract_user_text(content_val);
                if text.is_empty() {
                    continue;
                }

                let turn = Turn {
                    index: turns.len() as u32,
                    role: Role::User,
                    timestamp: ts,
                    content: text,
                    actions: Vec::new(),
                    tokens: None,
                    thinking: None,
                    is_sidechain,
                };
                turns.push(turn);
            }

            "assistant" => {
                let message = &value["message"];

                // Extract model
                if model.is_none() {
                    model = message["model"].as_str().map(String::from);
                }

                let is_sidechain = value["isSidechain"].as_bool().unwrap_or(false);

                // Parse usage
                let usage = &message["usage"];
                let tokens = if !usage.is_null() {
                    let input = usage["input_tokens"].as_u64().unwrap_or(0);
                    let output = usage["output_tokens"].as_u64().unwrap_or(0);
                    let cached = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                    total_tokens.input += input;
                    total_tokens.output += output;
                    total_tokens.cached += cached;
                    Some(TokenUsage {
                        input,
                        output,
                        cached,
                    })
                } else {
                    None
                };

                // Parse content array
                let mut text_parts: Vec<String> = Vec::new();
                let mut actions: Vec<Action> = Vec::new();
                let mut thinking_parts: Vec<String> = Vec::new();
                let mut new_pending: HashMap<String, usize> = HashMap::new();

                if let Some(content_arr) = message["content"].as_array() {
                    for item in content_arr {
                        match item["type"].as_str() {
                            Some("text") => {
                                if let Some(t) = item["text"].as_str() {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("thinking") => {
                                if let Some(t) = item["thinking"].as_str() {
                                    thinking_parts.push(t.to_string());
                                }
                            }
                            Some("tool_use") => {
                                let name = item["name"].as_str().unwrap_or("unknown").to_string();
                                let tool_use_id = item["id"].as_str().unwrap_or("").to_string();
                                let input_summary = summarize_tool_input(&name, &item["input"]);

                                let action_idx = actions.len();
                                if !tool_use_id.is_empty() {
                                    new_pending.insert(tool_use_id.clone(), action_idx);
                                }

                                actions.push(Action::ToolUse {
                                    name,
                                    input_summary,
                                    output_summary: String::new(),
                                    tool_use_id: Some(tool_use_id),
                                });
                            }
                            _ => {}
                        }
                    }
                }

                pending_tool_uses = new_pending;

                let content = text_parts.join("\n\n");
                let thinking = if thinking_parts.is_empty() {
                    None
                } else {
                    Some(thinking_parts.join("\n\n"))
                };

                let turn = Turn {
                    index: turns.len() as u32,
                    role: Role::Assistant,
                    timestamp: ts,
                    content,
                    actions,
                    tokens,
                    thinking,
                    is_sidechain,
                };
                turns.push(turn);
            }

            // Skip non-conversation message types
            "queue-operation" | "attachment" | "last-prompt" => continue,
            _ => continue,
        }
    }

    if line_count == 0 {
        return Err(anyhow!("empty session file"));
    }

    let id = session_id
        .or_else(|| {
            // Derive from filename if not in content
            path.file_stem().and_then(|s| s.to_str()).map(String::from)
        })
        .unwrap_or_else(|| uuid_from_path(path));

    // Derive project from cwd
    let project = cwd
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(String::from);

    let start_time = first_timestamp.unwrap_or_else(chrono::Utc::now);
    let end_time = last_timestamp;

    Ok(Session {
        id,
        agent: AgentKind::ClaudeCode,
        model,
        project,
        cwd,
        git_branch,
        host: Some(gethostname::gethostname().to_string_lossy().to_string()),
        start_time,
        end_time,
        turns,
        total_tokens,
    })
}

fn extract_user_text(content: &Value) -> String {
    if content.is_string() {
        return content.as_str().unwrap_or("").to_string();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|item| {
                if item["type"].as_str() == Some("text") {
                    item["text"].as_str().map(String::from)
                } else {
                    None
                }
            })
            .collect();
        return parts.join("\n");
    }
    String::new()
}

fn extract_tool_result_content(content: &Value) -> String {
    if content.is_string() {
        return content.as_str().unwrap_or("").to_string();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|item| {
                if item["type"].as_str() == Some("text") {
                    item["text"].as_str().map(String::from)
                } else {
                    None
                }
            })
            .collect();
        return parts.join("\n");
    }
    String::new()
}

fn summarize_tool_input(tool_name: &str, input: &Value) -> String {
    match tool_name {
        "Bash" | "bash" => input["command"].as_str().unwrap_or("").to_string(),
        "Read" | "read" => input["file_path"].as_str().unwrap_or("").to_string(),
        "Edit" | "edit" | "MultiEdit" => input["file_path"].as_str().unwrap_or("").to_string(),
        "Write" | "write" => input["file_path"].as_str().unwrap_or("").to_string(),
        "Grep" | "grep" => {
            let pattern = input["pattern"].as_str().unwrap_or("");
            let path = input["path"].as_str().unwrap_or("");
            format!("{pattern} in {path}")
        }
        "Glob" | "glob" => input["pattern"].as_str().unwrap_or("").to_string(),
        _ => {
            // Generic: show first 200 chars of JSON
            let s = input.to_string();
            truncate_str(&s, 200)
        }
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let truncated: String = chars[..max].iter().collect();
        format!("{}...", truncated)
    }
}

fn uuid_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_jsonl(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f
    }

    #[test]
    fn test_parse_basic_user_assistant() {
        let lines = &[
            r#"{"type":"user","message":{"role":"user","content":"Hello there"},"timestamp":"2026-04-05T10:00:00Z","sessionId":"test-session-123","cwd":"/Users/user/myproject","gitBranch":"main","version":"1.0"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","id":"msg_1","model":"claude-opus-4-6","content":[{"type":"text","text":"Hello! How can I help?"}],"usage":{"input_tokens":5,"output_tokens":10,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}},"timestamp":"2026-04-05T10:00:01Z"}"#,
        ];
        let f = write_jsonl(lines);
        let session = parse_claude_jsonl(f.path()).unwrap();
        assert_eq!(session.id, "test-session-123");
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert_eq!(session.model.as_deref(), Some("claude-opus-4-6"));
    }

    #[test]
    fn test_parse_tool_use() {
        let lines = &[
            r#"{"type":"user","message":{"role":"user","content":"Run ls"},"timestamp":"2026-04-05T10:00:00Z","sessionId":"s1","cwd":"/tmp","gitBranch":"main","version":"1.0"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","id":"msg_1","model":"claude","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls -la","description":"List files"}}],"usage":{"input_tokens":5,"output_tokens":3}},"timestamp":"2026-04-05T10:00:01Z"}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"file1.txt\nfile2.txt","is_error":false}]},"timestamp":"2026-04-05T10:00:02Z"}"#,
        ];
        let f = write_jsonl(lines);
        let session = parse_claude_jsonl(f.path()).unwrap();
        // user + assistant (tool_result doesn't create new turn)
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[1].actions.len(), 1);
        if let Action::ToolUse {
            name,
            output_summary,
            ..
        } = &session.turns[1].actions[0]
        {
            assert_eq!(name, "Bash");
            assert!(output_summary.contains("file1.txt"));
        }
    }

    #[test]
    fn test_parse_thinking_block() {
        let lines = &[
            r#"{"type":"user","message":{"role":"user","content":"Think about this"},"timestamp":"2026-04-05T10:00:00Z","sessionId":"s2","cwd":"/tmp","gitBranch":"main","version":"1.0"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","id":"msg_1","model":"claude","content":[{"type":"thinking","thinking":"Let me reason..."},{"type":"text","text":"Here is my answer"}],"usage":{"input_tokens":5,"output_tokens":8}},"timestamp":"2026-04-05T10:00:01Z"}"#,
        ];
        let f = write_jsonl(lines);
        let session = parse_claude_jsonl(f.path()).unwrap();
        assert_eq!(
            session.turns[1].thinking.as_deref(),
            Some("Let me reason...")
        );
        assert!(session.turns[1].content.contains("Here is my answer"));
    }

    #[test]
    fn test_skip_invalid_lines() {
        let lines = &[
            r#"{"type":"user","message":{"role":"user","content":"Hello"},"timestamp":"2026-04-05T10:00:00Z","sessionId":"s3","cwd":"/tmp","gitBranch":"main","version":"1.0"}"#,
            r#"INVALID JSON LINE"#,
            r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-04-05T10:00:01Z"}"#,
        ];
        let f = write_jsonl(lines);
        let session = parse_claude_jsonl(f.path()).unwrap();
        assert_eq!(session.turns.len(), 1); // Only the valid user turn
    }

    #[test]
    fn test_empty_file_returns_err() {
        let f = write_jsonl(&[]);
        let result = parse_claude_jsonl(f.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_token_aggregation() {
        let lines = &[
            r#"{"type":"user","message":{"role":"user","content":"Q1"},"timestamp":"2026-04-05T10:00:00Z","sessionId":"s4","cwd":"/tmp","gitBranch":"main","version":"1.0"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","id":"m1","model":"claude","content":[{"type":"text","text":"A1"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":200}},"timestamp":"2026-04-05T10:00:01Z"}"#,
        ];
        let f = write_jsonl(lines);
        let session = parse_claude_jsonl(f.path()).unwrap();
        assert_eq!(session.total_tokens.input, 100);
        assert_eq!(session.total_tokens.output, 50);
        assert_eq!(session.total_tokens.cached, 200);
    }
}
