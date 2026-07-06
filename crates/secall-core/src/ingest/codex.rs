use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::types::{Action, AgentKind, Role, Session, Turn};
use super::SessionParser;

pub struct CodexParser;

impl SessionParser for CodexParser {
    fn can_parse(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        (path_str.contains("/.codex/sessions/") || path_str.contains("\\.codex\\sessions\\"))
            && path.extension().map(|e| e == "jsonl").unwrap_or(false)
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        parse_codex_jsonl(path).map_err(|e| crate::error::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e,
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::Codex
    }
}

/// 최상위 JSONL 라인 — type + payload
#[derive(Deserialize)]
struct JsonlLine {
    #[serde(rename = "type")]
    line_type: String,
    #[serde(default)]
    payload: serde_json::Value,
    #[serde(default)]
    timestamp: Option<String>,
}

/// session_meta payload
#[derive(Deserialize)]
struct SessionMeta {
    id: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    model_provider: Option<String>,
}

/// response_item payload (untagged — type 필드로 수동 분기)
#[derive(Deserialize)]
struct ResponsePayload {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: serde_json::Value,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    call_id: Option<String>,
    #[serde(default)]
    arguments: serde_json::Value,
    #[serde(default)]
    output: serde_json::Value,
}

pub fn parse_codex_jsonl(path: &Path) -> Result<Session> {
    // Extract session ID from filename: rollout-<uuid>.jsonl → uuid
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.strip_prefix("rollout-").unwrap_or(s).to_string())
        .ok_or_else(|| anyhow!("invalid codex session filename: {}", path.display()))?;

    let file = std::fs::File::open(path)?;
    let file_mtime = file
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|st| {
            let duration = st.duration_since(std::time::UNIX_EPOCH).ok()?;
            DateTime::from_timestamp(duration.as_secs() as i64, duration.subsec_nanos())
        });
    let reader = std::io::BufReader::new(file);

    let mut turns: Vec<Turn> = Vec::new();
    let mut pending_calls: HashMap<String, (usize, usize)> = HashMap::new();
    let mut turn_idx: u32 = 0;

    // session_meta에서 추출
    let mut meta_id: Option<String> = None;
    let mut meta_timestamp: Option<DateTime<Utc>> = None;
    let mut meta_cwd: Option<String> = None;

    for line_result in reader.lines() {
        let line = line_result?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let jl: JsonlLine = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match jl.line_type.as_str() {
            "session_meta" => {
                if let Ok(meta) = serde_json::from_value::<SessionMeta>(jl.payload) {
                    meta_id = Some(meta.id);
                    meta_cwd = meta.cwd;
                    meta_timestamp = meta
                        .timestamp
                        .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                        .map(|dt| dt.with_timezone(&Utc));
                }
            }
            "response_item" => {
                let rp: ResponsePayload = match serde_json::from_value(jl.payload) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                match rp.item_type.as_str() {
                    "message" => {
                        let role_str = rp.role.as_deref().unwrap_or("");
                        // developer = 시스템 프롬프트 → skip
                        if role_str == "developer" {
                            continue;
                        }

                        let role = match role_str {
                            "user" => Role::User,
                            "assistant" => Role::Assistant,
                            _ => continue,
                        };

                        let content = extract_content(&rp.content);
                        if role == Role::User && content.is_empty() {
                            continue;
                        }

                        // 턴 타임스탬프: 래퍼의 timestamp 필드
                        let ts = jl
                            .timestamp
                            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                            .map(|dt| dt.with_timezone(&Utc));

                        // codex는 assistant 응답의 텍스트를 reasoning/function_call
                        // 뒤(응답 끝)에 emit한다. 앞선 reasoning/function_call이 이미
                        // 열어둔 (아직 텍스트가 없는) assistant 턴이 있으면 새 턴을
                        // 만들지 않고 이 텍스트를 그 턴에 접는다.
                        if role == Role::Assistant {
                            if let Some(last) = turns.last_mut() {
                                if last.role == Role::Assistant && last.content.is_empty() {
                                    last.content = content;
                                    if last.timestamp.is_none() {
                                        last.timestamp = ts;
                                    }
                                    continue;
                                }
                            }
                        }

                        turns.push(Turn {
                            index: turn_idx,
                            role,
                            timestamp: ts,
                            content,
                            actions: Vec::new(),
                            tokens: None,
                            thinking: None,
                            is_sidechain: false,
                        });
                        turn_idx += 1;
                    }
                    "function_call" => {
                        let name = rp.name.unwrap_or_else(|| "unknown".to_string());
                        let call_id = rp.call_id.unwrap_or_default();
                        let arguments = value_to_string(rp.arguments);
                        let ts = jl
                            .timestamp
                            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                            .map(|dt| dt.with_timezone(&Utc));

                        // codex는 assistant 텍스트 없이 곧바로 tool을 호출할 수 있다.
                        // 이때 turns.last()는 직전 user 턴이므로, ToolUse가 user 턴에
                        // 잘못 붙지 않도록 assistant 턴을 확보해 거기에 붙인다.
                        let turn_pos = ensure_assistant_turn(&mut turns, &mut turn_idx, ts);
                        let action_idx = turns[turn_pos].actions.len();
                        turns[turn_pos].actions.push(Action::ToolUse {
                            name,
                            input_summary: arguments,
                            output_summary: String::new(),
                            tool_use_id: Some(call_id.clone()),
                        });
                        if !call_id.is_empty() {
                            pending_calls.insert(call_id, (turn_pos, action_idx));
                        }
                    }
                    "function_call_output" => {
                        let call_id = rp.call_id.unwrap_or_default();
                        let output = value_to_string(rp.output);

                        if let Some((turn_pos, action_idx)) = pending_calls.remove(&call_id) {
                            if let Some(turn) = turns.get_mut(turn_pos) {
                                if let Some(Action::ToolUse { output_summary, .. }) =
                                    turn.actions.get_mut(action_idx)
                                {
                                    *output_summary = output;
                                }
                            }
                        }
                    }
                    "reasoning" => {
                        // reasoning은 assistant 응답의 시작 신호다. 뒤따르는
                        // function_call / assistant message가 붙을 assistant 턴을
                        // 미리 연다 (last가 이미 assistant면 no-op).
                        let ts = jl
                            .timestamp
                            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
                            .map(|dt| dt.with_timezone(&Utc));
                        ensure_assistant_turn(&mut turns, &mut turn_idx, ts);
                    }
                    // 그 외 아이템 → skip
                    _ => {}
                }
            }
            // "event_msg", "turn_context" 등 → skip
            _ => {}
        }
    }

    if turns.is_empty() {
        return Err(anyhow!(
            "codex session has no parseable turns: {}",
            path.display()
        ));
    }

    // session_meta의 id가 있으면 우선 사용 (filename fallback)
    let final_id = meta_id.unwrap_or(session_id);

    // cwd에서 프로젝트명 추출: "/Users/d9ng/proj/seCall" → "seCall"
    let project = meta_cwd
        .as_deref()
        .and_then(|p| Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    // start_time 우선순위: session_meta.timestamp > file mtime > Utc::now()
    let start_time = meta_timestamp.or(file_mtime).unwrap_or_else(Utc::now);

    Ok(Session {
        id: final_id,
        agent: AgentKind::Codex,
        model: None,
        project,
        cwd: meta_cwd.map(|s| s.into()),
        git_branch: None,
        host: Some(gethostname::gethostname().to_string_lossy().to_string()),
        start_time,
        end_time: None,
        turns,
        total_tokens: Default::default(),
        session_type: "interactive".to_string(),
        archived: false,
        archived_at: None,
    })
}

/// Return the index of the assistant turn that the current codex response is
/// building. Codex emits reasoning / function_call items before (or entirely
/// without) the assistant message text, so if the most recent turn is not an
/// assistant turn we open a fresh (empty) one — otherwise tool calls would be
/// misattributed to the preceding user turn.
fn ensure_assistant_turn(
    turns: &mut Vec<Turn>,
    turn_idx: &mut u32,
    ts: Option<DateTime<Utc>>,
) -> usize {
    if let Some(last) = turns.last() {
        if last.role == Role::Assistant {
            return turns.len() - 1;
        }
    }
    turns.push(Turn {
        index: *turn_idx,
        role: Role::Assistant,
        timestamp: ts,
        content: String::new(),
        actions: Vec::new(),
        tokens: None,
        thinking: None,
        is_sidechain: false,
    });
    *turn_idx += 1;
    turns.len() - 1
}

/// Convert a serde_json::Value to String — strings pass through, others serialize to JSON.
fn value_to_string(v: serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Extract plain text from Codex message content (string or parts array)
fn extract_content(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| {
                let t = v.get("type").and_then(|t| t.as_str())?;
                // input_text (user), output_text (assistant) 모두 처리
                if t == "input_text" || t == "output_text" {
                    v.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    fn make_codex_file(lines: &[&str]) -> tempfile::NamedTempFile {
        let mut f = Builder::new()
            .prefix("rollout-test-uuid-")
            .suffix(".jsonl")
            .tempfile()
            .unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        f
    }

    #[test]
    fn test_codex_parse_basic() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"test-uuid","timestamp":"2026-04-05T10:00:00Z","cwd":"/Users/test/proj/myapp"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"검색 기능 구현해줘"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"구현하겠습니다"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert!(session.turns[0].content.contains("검색"));
        assert_eq!(session.agent, AgentKind::Codex);
    }

    #[test]
    fn test_codex_function_call_matching() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"test-uuid"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"ls 실행"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"실행합니다"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"command\":\"ls\"}"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"file1.rs\nfile2.rs"}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        let assistant = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        assert_eq!(assistant.actions.len(), 1);
        match &assistant.actions[0] {
            Action::ToolUse {
                name,
                output_summary,
                ..
            } => {
                assert_eq!(name, "shell");
                assert!(output_summary.contains("file1.rs"));
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_codex_detect_path() {
        let parser = CodexParser;
        let p = Path::new("/Users/user/.codex/sessions/2026/04/06/rollout-abc.jsonl");
        assert!(parser.can_parse(p));
        let p2 = Path::new("/Users/user/.claude/projects/proj/session.jsonl");
        assert!(!parser.can_parse(p2));
        // non-jsonl extension should not match
        let p3 = Path::new("/Users/user/.codex/sessions/2026/04/06/rollout-abc.json");
        assert!(!parser.can_parse(p3));
    }

    #[test]
    fn test_codex_detect_content() {
        // Content sniffing: "type" = "response_item" + "payload" object → Codex
        let line = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"test"}]}}"#;
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(v["type"].as_str().unwrap(), "response_item");
        assert!(v["payload"].is_object());
    }

    #[test]
    fn test_codex_timestamp_from_meta() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"ts-test","timestamp":"2026-04-05T10:00:00Z","cwd":"/tmp"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        // session_meta.timestamp가 start_time으로 사용되어야 함
        assert_eq!(
            session.start_time,
            DateTime::parse_from_rfc3339("2026-04-05T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn test_codex_session_id_from_meta() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"meta-uuid-123"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        // session_meta.id가 filename보다 우선
        assert_eq!(session.id, "meta-uuid-123");
    }

    #[test]
    fn test_codex_project_from_cwd() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"cwd-test","cwd":"/Users/d9ng/proj/seCall"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        assert_eq!(session.project.as_deref(), Some("seCall"));
        assert_eq!(
            session
                .cwd
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            Some("/Users/d9ng/proj/seCall".to_string())
        );
    }

    #[test]
    fn test_codex_skip_developer_and_reasoning() {
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"skip-test"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:00Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"system prompt"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"reasoning","content":null,"summary":[]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        // developer와 reasoning은 skip → user + assistant = 2 turns
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Assistant);
    }

    #[test]
    fn test_codex_arguments_as_object() {
        // arguments가 JSON 객체로 올 때도 파싱 실패 없이 처리
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"obj-args"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"test"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"ok"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"container","call_id":"call-obj","arguments":{"image":"node:18","command":"npm test"}}}"#,
            r#"{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-obj","output":["line1","line2"]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        let assistant = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        assert_eq!(assistant.actions.len(), 1);
        match &assistant.actions[0] {
            Action::ToolUse {
                name,
                input_summary,
                output_summary,
                ..
            } => {
                assert_eq!(name, "container");
                // 객체가 JSON 문자열로 저장됨
                assert!(input_summary.contains("node:18"));
                // 배열이 JSON 문자열로 저장됨
                assert!(output_summary.contains("line1"));
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_codex_tool_call_attaches_to_assistant_not_user() {
        // 실제 codex: leading assistant message 없이
        // reasoning -> function_call -> function_call_output 로 tool을 호출.
        // ToolUse는 반드시 assistant 턴에 붙어야 하고, 직전 user 턴에 새면 안 된다.
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"tool-first"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"ls 실행"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"reasoning","content":null,"summary":[]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"call-1","arguments":"{\"command\":\"ls\"}"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-1","output":"file1.rs\nfile2.rs"}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();

        // user 턴에는 tool action이 전혀 없어야 한다 (오귀속 방지)
        let user = session.turns.iter().find(|t| t.role == Role::User).unwrap();
        assert!(
            user.actions.is_empty(),
            "tool call leaked onto the user turn"
        );

        // assistant 턴이 존재하고 ToolUse(+resolved output)를 보유해야 한다
        let assistant = session
            .turns
            .iter()
            .find(|t| t.role == Role::Assistant)
            .unwrap();
        assert_eq!(assistant.actions.len(), 1);
        match &assistant.actions[0] {
            Action::ToolUse {
                name,
                output_summary,
                ..
            } => {
                assert_eq!(name, "shell");
                assert!(output_summary.contains("file1.rs"));
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_codex_normal_message_first_response() {
        // assistant가 텍스트를 먼저 emit한 뒤 tool을 호출하는 정상 응답.
        // 텍스트와 ToolUse가 하나의 assistant 턴에 담겨야 하고,
        // 빈 중복 assistant 턴이 생기면 안 된다.
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"msg-first"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"검색"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"실행합니다"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"call-2","arguments":"{\"command\":\"ls\"}"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"call-2","output":"ok"}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        let assistant_turns: Vec<_> = session
            .turns
            .iter()
            .filter(|t| t.role == Role::Assistant)
            .collect();
        assert_eq!(
            assistant_turns.len(),
            1,
            "should not create duplicate assistant turns"
        );
        let a = assistant_turns[0];
        assert!(a.content.contains("실행합니다"));
        assert_eq!(a.actions.len(), 1);
        match &a.actions[0] {
            Action::ToolUse {
                name,
                output_summary,
                ..
            } => {
                assert_eq!(name, "shell");
                assert_eq!(output_summary, "ok");
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn test_codex_tool_first_then_trailing_message() {
        // reasoning -> function_call -> output -> assistant message(text) 순서에서
        // 마지막 텍스트가 같은 assistant 턴에 접혀야 한다.
        let f = make_codex_file(&[
            r#"{"type":"session_meta","payload":{"id":"trailing"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:01Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"go"}]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:02Z","type":"response_item","payload":{"type":"reasoning","content":null,"summary":[]}}"#,
            r#"{"timestamp":"2026-04-05T10:00:03Z","type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"c1","arguments":"{}"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:04Z","type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"done"}}"#,
            r#"{"timestamp":"2026-04-05T10:00:05Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"완료했습니다"}]}}"#,
        ]);
        let session = parse_codex_jsonl(f.path()).unwrap();
        assert_eq!(session.turns.len(), 2, "user + one folded assistant turn");
        let a = &session.turns[1];
        assert_eq!(a.role, Role::Assistant);
        assert!(a.content.contains("완료"));
        assert_eq!(a.actions.len(), 1);
    }
}
