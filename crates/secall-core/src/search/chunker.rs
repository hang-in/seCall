use crate::ingest::Session;

const MAX_CHUNK_CHARS: usize = 3600;
const OVERLAP_CHARS: usize = 540; // ~15%

#[derive(Debug, Clone)]
pub struct Chunk {
    pub session_id: String,
    pub turn_index: u32,
    pub seq: u32,
    pub text: String,
    pub context: String,
}

pub fn chunk_session(session: &Session) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    for turn in &session.turns {
        let context = format!(
            "Session: {} {} {} | Turn {}: {}",
            session.agent.as_str(),
            session.project.as_deref().unwrap_or("unknown"),
            session.start_time.format("%Y-%m-%d"),
            turn.index + 1,
            turn.role.as_str(),
        );

        let text = build_turn_text(turn);
        if text.is_empty() {
            continue;
        }

        if text.len() <= MAX_CHUNK_CHARS {
            chunks.push(Chunk {
                session_id: session.id.clone(),
                turn_index: turn.index,
                seq: 0,
                text,
                context: context.clone(),
            });
        } else {
            let turn_chunks = split_into_chunks(&text, MAX_CHUNK_CHARS, OVERLAP_CHARS);
            for (seq, chunk_text) in turn_chunks.into_iter().enumerate() {
                chunks.push(Chunk {
                    session_id: session.id.clone(),
                    turn_index: turn.index,
                    seq: seq as u32,
                    text: chunk_text,
                    context: context.clone(),
                });
            }
        }
    }

    chunks
}

fn build_turn_text(turn: &crate::ingest::Turn) -> String {
    let mut parts = Vec::new();

    if !turn.content.is_empty() {
        parts.push(turn.content.clone());
    }

    if let Some(thinking) = &turn.thinking {
        parts.push(thinking.clone());
    }

    for action in &turn.actions {
        if let crate::ingest::Action::ToolUse {
            name,
            input_summary,
            output_summary,
            ..
        } = action
        {
            parts.push(format!(
                "[Tool: {}] {} {}",
                name, input_summary, output_summary
            ));
        }
    }

    parts.join("\n\n")
}

fn split_into_chunks(text: &str, max_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();

    if total <= max_size {
        return vec![text.to_string()];
    }

    let mut start = 0;
    while start < total {
        let end = (start + max_size).min(total);
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);

        if end >= total {
            break;
        }
        // Advance with overlap
        start = end.saturating_sub(overlap);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use chrono::{TimeZone, Utc};

    fn make_session(turns: Vec<Turn>) -> Session {
        Session {
            id: "test-session".to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: Some("testproj".to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc.with_ymd_and_hms(2026, 4, 5, 0, 0, 0).unwrap(),
            end_time: None,
            turns,
            total_tokens: TokenUsage::default(),
        }
    }

    #[test]
    fn test_short_turn_single_chunk() {
        let turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: "Short content".to_string(),
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let chunks = chunk_session(&session);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].seq, 0);
    }

    #[test]
    fn test_long_turn_split() {
        let long_content = "word ".repeat(1000);
        let turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: long_content,
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let chunks = chunk_session(&session);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_context_string_correct() {
        let turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: "Test".to_string(),
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let chunks = chunk_session(&session);
        assert!(chunks[0].context.contains("claude-code"));
        assert!(chunks[0].context.contains("testproj"));
    }

    #[test]
    fn test_overlap_in_long_turn() {
        // Long content that generates multiple chunks
        let content: String = (0..1000).map(|i| format!("word{} ", i)).collect();
        let turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content,
            actions: Vec::new(),
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        let session = make_session(turns);
        let chunks = chunk_session(&session);
        if chunks.len() > 1 {
            // Each chunk should have seq increasing
            for (i, chunk) in chunks.iter().enumerate() {
                assert_eq!(chunk.seq, i as u32);
            }
        }
    }
}
