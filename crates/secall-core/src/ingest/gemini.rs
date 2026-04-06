use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::types::{Action, AgentKind, Role, Session, TokenUsage, Turn};
use super::SessionParser;

pub struct GeminiParser;

impl SessionParser for GeminiParser {
    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        (path_str.contains("/.gemini/") || path_str.contains("\\.gemini\\"))
            && path.extension().map(|e| e == "json").unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        parse_gemini_json(path).map_err(|e| crate::error::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e,
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::GeminiCli
    }
}

// ─── Serde models ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSession {
    session_id: String,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    last_updated: Option<String>,
    #[serde(default)]
    messages: Vec<GeminiMessage>,
    // projectHash, kind — 무시 (serde(default)로 skip)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiMessage {
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    content: serde_json::Value, // array [{text}] 또는 string
    #[serde(default)]
    thoughts: Option<Vec<GeminiThought>>,
    #[serde(default)]
    tokens: Option<GeminiTokens>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<GeminiToolCall>>,
}

#[derive(Deserialize)]
struct GeminiThought {
    #[serde(default)]
    _subject: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct GeminiTokens {
    #[serde(default)]
    input: u64,
    #[serde(default)]
    output: u64,
    #[serde(default)]
    cached: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiToolCall {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    args: serde_json::Value,
    #[serde(default)]
    result: Option<Vec<GeminiToolResult>>,
    #[serde(default)]
    _status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiToolResult {
    #[serde(default)]
    function_response: Option<GeminiFunctionResponse>,
}

#[derive(Deserialize)]
struct GeminiFunctionResponse {
    #[serde(default)]
    response: serde_json::Value,
}

// ─── Parser ───────────────────────────────────────────────────────────────────

pub fn parse_gemini_json(path: &Path) -> Result<Session> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 100 * 1024 * 1024 {
        tracing::warn!(
            size_mb = metadata.len() / 1024 / 1024,
            path = %path.display(),
            "gemini session file is large"
        );
    }

    let raw = std::fs::read_to_string(path)?;
    let gs: GeminiSession = serde_json::from_str(&raw)
        .map_err(|e| anyhow!("failed to parse gemini session {}: {e}", path.display()))?;

    let project = extract_project_id(path);

    let mut turns: Vec<Turn> = Vec::new();
    let mut turn_idx: u32 = 0;
    let mut session_model: Option<String> = None;

    for msg in &gs.messages {
        // 턴 타임스탬프
        let ts = msg
            .timestamp
            .as_deref()
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|dt| dt.with_timezone(&Utc));

        match msg.msg_type.as_str() {
            "user" => {
                let content = extract_content(&msg.content);
                if !content.is_empty() {
                    turns.push(Turn {
                        index: turn_idx,
                        role: Role::User,
                        timestamp: ts,
                        content,
                        actions: Vec::new(),
                        tokens: None,
                        thinking: None,
                        is_sidechain: false,
                    });
                    turn_idx += 1;
                }
            }
            "gemini" => {
                let content = extract_content(&msg.content);

                // model — 첫 번째 gemini 메시지에서 추출
                if session_model.is_none() {
                    session_model = msg.model.clone();
                }

                // thinking — thoughts[].description 결합
                let thinking = msg.thoughts.as_ref().and_then(|thoughts| {
                    let text: Vec<String> = thoughts
                        .iter()
                        .filter_map(|t| t.description.clone())
                        .collect();
                    if text.is_empty() {
                        None
                    } else {
                        Some(text.join("\n\n"))
                    }
                });

                // tokens
                let tokens = msg.tokens.as_ref().map(|t| TokenUsage {
                    input: t.input,
                    output: t.output,
                    cached: t.cached,
                });

                // toolCalls → actions
                let mut actions = Vec::new();
                if let Some(tool_calls) = &msg.tool_calls {
                    for tc in tool_calls {
                        let name = tc.name.clone().unwrap_or_else(|| "unknown".to_string());
                        let input_summary = tc.args.to_string();

                        // result[0].functionResponse.response에서 output 추출
                        let output_summary = tc
                            .result
                            .as_ref()
                            .and_then(|results| results.first())
                            .and_then(|r| r.function_response.as_ref())
                            .map(|fr| {
                                // response.output 또는 response.error 추출
                                fr.response
                                    .get("output")
                                    .or_else(|| fr.response.get("error"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            })
                            .unwrap_or_default();

                        actions.push(Action::ToolUse {
                            name,
                            input_summary,
                            output_summary,
                            tool_use_id: None,
                        });
                    }
                }

                turns.push(Turn {
                    index: turn_idx,
                    role: Role::Assistant,
                    timestamp: ts,
                    content,
                    actions,
                    tokens,
                    thinking,
                    is_sidechain: false,
                });
                turn_idx += 1;
            }
            // "info" 등 → skip
            _ => {}
        }
    }

    if turns.is_empty() {
        return Err(anyhow!(
            "gemini session has no parseable turns: {}",
            path.display()
        ));
    }

    let start_time = gs
        .start_time
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let end_time = gs
        .last_updated
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Session {
        id: gs.session_id,
        agent: AgentKind::GeminiCli,
        model: session_model,
        project,
        cwd: None,
        git_branch: None,
        host: Some(gethostname::gethostname().to_string_lossy().to_string()),
        start_time,
        end_time,
        turns,
        total_tokens: Default::default(),
    })
}

/// content가 string이면 그대로, array이면 [{text: "..."}]에서 text 추출
fn extract_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Extract projectId from path: ~/.gemini/tmp/<projectId>/chats/session-*.json
/// Supports both Unix (`/.gemini/tmp/`) and Windows (`\.gemini\tmp\`) separators.
fn extract_project_id(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    // Try Unix separator first, then Windows
    for marker in &["/.gemini/tmp/", "\\.gemini\\tmp\\"] {
        if let Some(pos) = path_str.find(marker) {
            let after = &path_str[pos + marker.len()..];
            // Split on either separator
            let end = after.find(['/', '\\']).unwrap_or(after.len());
            let project_id = &after[..end];
            if !project_id.is_empty() {
                return Some(project_id.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    fn make_gemini_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = Builder::new()
            .prefix("session-test-")
            .suffix(".json")
            .tempfile()
            .unwrap();
        write!(f, "{content}").unwrap();
        f
    }

    const BASIC_SESSION: &str = r#"{
        "sessionId": "test-uuid",
        "startTime": "2026-04-05T10:00:00Z",
        "lastUpdated": "2026-04-05T10:30:00Z",
        "kind": "main",
        "projectHash": "abc123",
        "messages": [
            {"id":"m1","timestamp":"2026-04-05T10:00:01Z","type":"user","content":[{"text":"검색 기능 구현해줘"}]},
            {"id":"m2","timestamp":"2026-04-05T10:00:02Z","type":"gemini","content":"네, 구현하겠습니다.","model":"gemini-3.1-pro","tokens":{"input":100,"output":50,"cached":0,"thoughts":0,"tool":0,"total":150}}
        ]
    }"#;

    #[test]
    fn test_gemini_parse_basic() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        assert_eq!(session.id, "test-uuid");
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert!(session.turns[0].content.contains("검색"));
        assert_eq!(session.agent, AgentKind::GeminiCli);
    }

    #[test]
    fn test_gemini_tool_calls() {
        let json = r#"{
            "sessionId": "s-tools",
            "startTime": "2026-04-05T10:00:00Z",
            "messages": [
                {"id":"m1","timestamp":"2026-04-05T10:00:01Z","type":"user","content":[{"text":"파일 읽어줘"}]},
                {"id":"m2","timestamp":"2026-04-05T10:00:02Z","type":"gemini","content":"","toolCalls":[
                    {"name":"read_file","args":{"file_path":"main.rs"},"result":[{"functionResponse":{"name":"read_file","response":{"output":"fn main() {}"}}}],"status":"success"}
                ],"tokens":{"input":200,"output":10,"cached":0,"thoughts":0,"tool":0,"total":210}}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        let asst = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        assert_eq!(asst.actions.len(), 1);
        match &asst.actions[0] {
            Action::ToolUse {
                name,
                output_summary,
                ..
            } => {
                assert_eq!(name, "read_file");
                assert!(
                    output_summary.contains("fn main()"),
                    "got: {output_summary}"
                );
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_gemini_thinking() {
        let json = r#"{
            "sessionId": "s-thinking",
            "startTime": "2026-04-05T10:00:00Z",
            "messages": [
                {"id":"m1","timestamp":"2026-04-05T10:00:01Z","type":"user","content":[{"text":"분석해줘"}]},
                {"id":"m2","timestamp":"2026-04-05T10:00:02Z","type":"gemini","content":"결과입니다.","thoughts":[
                    {"subject":"Analyzing","description":"코드를 분석하고 있습니다.","timestamp":"2026-04-05T10:00:03Z"},
                    {"subject":"Planning","description":"구현 계획을 세우고 있습니다.","timestamp":"2026-04-05T10:00:04Z"}
                ],"tokens":{"input":100,"output":30,"cached":0,"thoughts":50,"tool":0,"total":180}}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        let asst = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        let thinking = asst.thinking.as_ref().expect("thinking should be Some");
        assert!(thinking.contains("코드를 분석"), "got: {thinking}");
        assert!(thinking.contains("구현 계획"), "got: {thinking}");
    }

    #[test]
    fn test_gemini_tokens() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        let asst = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        let tokens = asst.tokens.as_ref().expect("tokens should be Some");
        assert_eq!(tokens.input, 100);
        assert_eq!(tokens.output, 50);
        assert_eq!(tokens.cached, 0);
    }

    #[test]
    fn test_gemini_model() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        assert_eq!(session.model.as_deref(), Some("gemini-3.1-pro"));
    }

    #[test]
    fn test_gemini_string_content() {
        let json = r#"{
            "sessionId": "s-string",
            "startTime": "2026-04-05T10:00:00Z",
            "messages": [
                {"id":"m1","type":"user","content":"직접 문자열 입력"},
                {"id":"m2","type":"gemini","content":"직접 문자열 응답","tokens":{"input":10,"output":5,"cached":0,"thoughts":0,"tool":0,"total":15}}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        assert_eq!(session.turns[0].content, "직접 문자열 입력");
        assert_eq!(session.turns[1].content, "직접 문자열 응답");
    }

    #[test]
    fn test_gemini_info_skip() {
        let json = r#"{
            "sessionId": "s-info",
            "startTime": "2026-04-05T10:00:00Z",
            "messages": [
                {"id":"m1","type":"user","content":[{"text":"hello"}]},
                {"id":"m2","type":"info","content":"Request cancelled."},
                {"id":"m3","type":"gemini","content":"hi","tokens":{"input":5,"output":3,"cached":0,"thoughts":0,"tool":0,"total":8}}
            ]
        }"#;
        let f = make_gemini_file(json);
        let session = parse_gemini_json(f.path()).unwrap();
        // info 메시지는 턴에 포함되지 않음 → 2 turns (user + gemini)
        assert_eq!(session.turns.len(), 2);
    }

    #[test]
    fn test_gemini_timestamps_parsed() {
        let f = make_gemini_file(BASIC_SESSION);
        let session = parse_gemini_json(f.path()).unwrap();
        // startTime "2026-04-05T10:00:00Z" → start_time
        assert_eq!(session.start_time.date_naive().to_string(), "2026-04-05");
        // lastUpdated "2026-04-05T10:30:00Z" → end_time
        assert!(session.end_time.is_some());
        assert_eq!(
            session.end_time.unwrap().date_naive().to_string(),
            "2026-04-05"
        );
    }

    #[test]
    fn test_gemini_detect_path() {
        let parser = GeminiParser;
        let p = Path::new("/Users/user/.gemini/tmp/proj123/chats/session-abc.json");
        assert!(parser.can_parse(p));
        let p2 = Path::new("/Users/user/.claude/projects/proj/session.jsonl");
        assert!(!parser.can_parse(p2));
        // must be .json not .jsonl
        let p3 = Path::new("/Users/user/.gemini/tmp/proj/chats/session.jsonl");
        assert!(!parser.can_parse(p3));
    }

    #[test]
    fn test_gemini_project_extraction() {
        let path = Path::new("/Users/user/.gemini/tmp/myproject123/chats/session-abc.json");
        let project = extract_project_id(path);
        assert_eq!(project, Some("myproject123".to_string()));

        let path2 = Path::new("/no/gemini/path/session.json");
        assert_eq!(extract_project_id(path2), None);
    }
}
