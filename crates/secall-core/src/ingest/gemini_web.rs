use std::io::{Cursor, Read};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::warn;

use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
use crate::ingest::SessionParser;

// ── serde 구조체 ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GeminiWebExport {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[allow(dead_code)]
    title: Option<String>,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "lastUpdated")]
    last_updated: String,
    messages: Vec<GeminiWebMessage>,
}

#[derive(Debug, Deserialize)]
struct GeminiWebMessage {
    #[allow(dead_code)]
    id: String,
    timestamp: String,
    #[serde(rename = "type")]
    msg_type: String,
    content: GeminiWebContent,
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GeminiWebContent {
    Text(String),
    Parts(Vec<GeminiWebPart>),
}

#[derive(Debug, Deserialize)]
struct GeminiWebPart {
    text: String,
}

// ── 변환 함수 ──────────────────────────────────────────────────────────────────

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn json_to_session(export: GeminiWebExport) -> Session {
    let start_time = parse_dt(&export.start_time);
    let end_time = Some(parse_dt(&export.last_updated));

    // 마지막 gemini 메시지의 model 필드
    let model = export
        .messages
        .iter()
        .rev()
        .find(|m| m.msg_type == "gemini")
        .and_then(|m| m.model.clone());

    let turns: Vec<Turn> = export
        .messages
        .into_iter()
        .enumerate()
        .map(|(idx, msg)| {
            let role = match msg.msg_type.as_str() {
                "user" => Role::User,
                "gemini" => Role::Assistant,
                _ => Role::System,
            };
            let content = match msg.content {
                GeminiWebContent::Text(s) => s,
                GeminiWebContent::Parts(parts) => {
                    parts.into_iter().map(|p| p.text).collect::<Vec<_>>().join("\n")
                }
            };
            let timestamp = DateTime::parse_from_rfc3339(&msg.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .ok();
            Turn {
                index: idx as u32,
                role,
                timestamp,
                content,
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            }
        })
        .collect();

    Session {
        id: export.session_id,
        agent: AgentKind::GeminiWeb,
        model,
        project: None,
        cwd: None,
        git_branch: None,
        host: None,
        start_time,
        end_time,
        turns,
        total_tokens: TokenUsage::default(),
        session_type: "interactive".to_string(),
    }
}

// ── 내부 헬퍼 (테스트에서도 사용 가능) ──────────────────────────────────────────

fn parse_archive<R: Read + std::io::Seek>(
    mut archive: zip::ZipArchive<R>,
) -> crate::error::Result<Vec<Session>> {
    let names: Vec<String> = archive.file_names().map(|s| s.to_owned()).collect();
    let mut sessions = Vec::new();

    for name in &names {
        if !name.ends_with(".json") {
            continue;
        }
        let mut file = match archive.by_name(name) {
            Ok(f) => f,
            Err(e) => {
                warn!("gemini_web: failed to open {} in ZIP: {}", name, e);
                continue;
            }
        };
        let mut raw = String::new();
        if let Err(e) = file.read_to_string(&mut raw) {
            warn!("gemini_web: failed to read {} in ZIP: {}", name, e);
            continue;
        }
        match serde_json::from_str::<GeminiWebExport>(&raw) {
            Ok(export) => sessions.push(json_to_session(export)),
            Err(e) => {
                warn!("gemini_web: failed to parse {} in ZIP: {}", name, e);
            }
        }
    }

    Ok(sessions)
}

// ── 파서 구현 ──────────────────────────────────────────────────────────────────

pub struct GeminiWebParser;

impl SessionParser for GeminiWebParser {
    fn can_parse(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("zip")
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        let sessions = self.parse_all(path)?;
        sessions.into_iter().next().ok_or_else(|| {
            crate::error::SecallError::UnsupportedFormat(
                "no sessions found in Gemini Web ZIP".to_string(),
            )
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::GeminiWeb
    }

    fn parse_all(&self, path: &Path) -> crate::error::Result<Vec<Session>> {
        let data = std::fs::read(path).map_err(crate::error::SecallError::VaultIo)?;
        let cursor = Cursor::new(data);
        let archive = zip::ZipArchive::new(cursor).map_err(|e| {
            crate::error::SecallError::Parse {
                path: path.to_string_lossy().to_string(),
                source: anyhow::anyhow!(e),
            }
        })?;
        parse_archive(archive)
    }
}

// ── 단위 테스트 ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const SAMPLE_JSON: &str = r#"{
  "sessionId": "22836c74f2ebe4cc",
  "title": "부천시 내일 날씨 예보",
  "startTime": "2025-07-24T08:19:17.000Z",
  "lastUpdated": "2025-07-24T08:19:33.000Z",
  "kind": "main",
  "projectHash": "gemini-web",
  "messages": [
    {
      "id": "msg-0",
      "timestamp": "2025-07-24T08:19:17.000Z",
      "type": "user",
      "content": [{ "text": "내일 날씨 알려줘" }]
    },
    {
      "id": "msg-1",
      "timestamp": "2025-07-24T08:19:17.000Z",
      "type": "gemini",
      "content": "내일 부천시는 맑은 하늘이 예상되며, 최고 기온은 34도입니다.",
      "model": "2.5 Flash"
    }
  ]
}"#;

    const SAMPLE_JSON2: &str = r#"{
  "sessionId": "aabbccdd11223344",
  "title": "다른 세션",
  "startTime": "2025-07-25T09:00:00.000Z",
  "lastUpdated": "2025-07-25T09:01:00.000Z",
  "kind": "main",
  "projectHash": "gemini-web",
  "messages": [
    {
      "id": "msg-0",
      "timestamp": "2025-07-25T09:00:00.000Z",
      "type": "user",
      "content": [{ "text": "안녕" }]
    },
    {
      "id": "msg-1",
      "timestamp": "2025-07-25T09:00:05.000Z",
      "type": "gemini",
      "content": "안녕하세요!",
      "model": "2.5 Flash"
    }
  ]
}"#;

    #[test]
    fn test_json_to_session_basic() {
        let export: GeminiWebExport = serde_json::from_str(SAMPLE_JSON).unwrap();
        let session = json_to_session(export);

        assert_eq!(session.id, "22836c74f2ebe4cc");
        assert_eq!(session.agent, AgentKind::GeminiWeb);
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[0].content, "내일 날씨 알려줘");
        assert_eq!(session.turns[1].role, Role::Assistant);
        assert_eq!(session.model, Some("2.5 Flash".to_string()));
    }

    #[test]
    fn test_parse_all_from_zip() {
        // 인메모리 ZIP 생성
        let mut buf = Vec::new();
        {
            let cursor = Cursor::new(&mut buf);
            let mut zip = zip::ZipWriter::new(cursor);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("session1.json", opts).unwrap();
            zip.write_all(SAMPLE_JSON.as_bytes()).unwrap();
            zip.start_file("session2.json", opts).unwrap();
            zip.write_all(SAMPLE_JSON2.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let archive = zip::ZipArchive::new(Cursor::new(&buf)).unwrap();
        let sessions = parse_archive(archive).unwrap();
        assert_eq!(sessions.len(), 2);
    }
}
