// Placeholder — will be fully defined after tokenizer builds
// to avoid circular compilation issues
use anyhow::Result;
use chrono::{DateTime, Utc};
use crate::error::SecallError;
use serde::Serialize;

use super::tokenizer::Tokenizer;
use crate::ingest::Session;
use crate::store::db::Database;
use crate::store::{SearchRepo, SessionRepo};

#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub turns_indexed: usize,
    pub chunks_embedded: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub agent: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub date: String,
    pub vault_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub session_id: String,
    pub turn_index: u32,
    pub score: f64,
    pub bm25_score: Option<f64>,
    pub vector_score: Option<f64>,
    pub snippet: String,
    pub metadata: SessionMeta,
}

#[derive(Debug)]
pub struct FtsRow {
    pub session_id: String,
    pub turn_index: u32,
    pub content: String,
    pub score: f64,
}

pub struct Bm25Indexer {
    tokenizer: Box<dyn Tokenizer>,
}

impl Bm25Indexer {
    pub fn new(tokenizer: Box<dyn Tokenizer>) -> Self {
        Bm25Indexer { tokenizer }
    }

    /// Index all turns of a session into the FTS5 table
    pub fn index_session(&self, db: &Database, session: &Session) -> Result<IndexStats> {
        let mut stats = IndexStats::default();

        // Insert session metadata first
        db.insert_session(session)?;

        for turn in &session.turns {
            // Tokenize turn content
            let tokenized = self.tokenizer.tokenize_for_fts(&turn.content);

            // Also tokenize thinking if present
            let full_text = if let Some(thinking) = &turn.thinking {
                format!(
                    "{} {}",
                    tokenized,
                    self.tokenizer.tokenize_for_fts(thinking)
                )
            } else {
                tokenized
            };

            db.insert_turn(&session.id, turn)?;
            db.insert_fts(&full_text, &session.id, turn.index)?;
            stats.turns_indexed += 1;
        }

        Ok(stats)
    }

    /// BM25 search via FTS5
    pub fn search(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<SearchResult>> {
        let tokenized_query = self.tokenizer.tokenize_for_fts(query);
        if tokenized_query.is_empty() {
            return Ok(Vec::new());
        }

        let fts_rows = db.search_fts(&tokenized_query, limit * 3, filters)?;
        if fts_rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut results: Vec<SearchResult> = fts_rows
            .into_iter()
            .filter_map(|row| {
                let snippet = extract_snippet(&row.content, query, 200);
                let session_meta = db.get_session_meta(&row.session_id).ok()?;

                // Apply project/agent filters (date already filtered in SQL)
                if let Some(proj) = &filters.project {
                    if session_meta.project.as_deref() != Some(proj.as_str()) {
                        return None;
                    }
                }
                if let Some(ag) = &filters.agent {
                    if session_meta.agent != *ag {
                        return None;
                    }
                }

                Some(SearchResult {
                    session_id: row.session_id,
                    turn_index: row.turn_index,
                    score: row.score,
                    bm25_score: Some(row.score),
                    vector_score: None,
                    snippet,
                    metadata: session_meta,
                })
            })
            .take(limit)
            .collect();

        normalize_scores(&mut results);
        Ok(results)
    }
}

fn normalize_scores(results: &mut [SearchResult]) {
    if results.is_empty() {
        return;
    }
    let max = results
        .iter()
        .map(|r| r.score)
        .fold(f64::NEG_INFINITY, f64::max);
    if max > 0.0 {
        for r in results.iter_mut() {
            r.score /= max;
        }
    }
}

fn extract_snippet(content: &str, query: &str, max_chars: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();

    if total <= max_chars {
        return content.to_string();
    }

    // Try to find the query in the content
    let lower_content: String = content.to_lowercase();
    let lower_query = query.to_lowercase();

    let start_char = if let Some(byte_pos) = lower_content.find(&lower_query) {
        // Convert byte position to char position
        let char_pos = content[..byte_pos].chars().count();
        char_pos.saturating_sub(30)
    } else {
        0
    };

    let end_char = (start_char + max_chars).min(total);
    let snippet: String = chars[start_char..end_char].iter().collect();
    snippet
}

// SessionRepo impl for Database — session/turn CRUD
impl SessionRepo for Database {
    fn insert_session(&self, session: &Session) -> crate::error::Result<()> {
        use chrono::Utc;
        let tools_used: Vec<String> = session
            .turns
            .iter()
            .flat_map(|t| &t.actions)
            .filter_map(|a| {
                if let crate::ingest::Action::ToolUse { name, .. } = a {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        self.conn().execute(
            "INSERT OR IGNORE INTO sessions(id, agent, model, project, cwd, git_branch, host, start_time, end_time, turn_count, tokens_in, tokens_out, tools_used, tags, ingested_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            rusqlite::params![
                session.id,
                session.agent.as_str(),
                session.model,
                session.project,
                session.cwd.as_ref().map(|p| p.to_string_lossy().to_string()),
                session.git_branch,
                session.host,
                session.start_time.to_rfc3339(),
                session.end_time.map(|t| t.to_rfc3339()),
                session.turns.len() as i64,
                session.total_tokens.input as i64,
                session.total_tokens.output as i64,
                serde_json::to_string(&tools_used).ok(),
                serde_json::to_string(&Vec::<String>::new()).ok(),
                Utc::now().to_rfc3339(),
                "raw",
            ],
        )?;
        Ok(())
    }

    fn update_session_vault_path(
        &self,
        session_id: &str,
        vault_path: &str,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            "UPDATE sessions SET vault_path = ?1, status = 'indexed' WHERE id = ?2",
            rusqlite::params![vault_path, session_id],
        )?;
        Ok(())
    }

    fn insert_turn(&self, session_id: &str, turn: &crate::ingest::Turn) -> crate::error::Result<i64> {
        let tool_names: Vec<String> = turn
            .actions
            .iter()
            .filter_map(|a| {
                if let crate::ingest::Action::ToolUse { name, .. } = a {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        let has_tool = !tool_names.is_empty();

        self.conn().execute(
            "INSERT OR IGNORE INTO turns(session_id, turn_index, role, timestamp, content, has_tool, tool_names, thinking, tokens_in, tokens_out)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                session_id,
                turn.index as i64,
                turn.role.as_str(),
                turn.timestamp.map(|t| t.to_rfc3339()),
                turn.content,
                has_tool as i64,
                serde_json::to_string(&tool_names).ok(),
                turn.thinking,
                turn.tokens.as_ref().map(|t| t.input as i64).unwrap_or(0),
                turn.tokens.as_ref().map(|t| t.output as i64).unwrap_or(0),
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    fn session_exists(&self, session_id: &str) -> crate::error::Result<bool> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn session_exists_by_prefix(&self, prefix: &str) -> crate::error::Result<bool> {
        let pattern = format!("{}%", prefix);
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE ?1",
            [pattern],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn get_session_meta(&self, session_id: &str) -> crate::error::Result<SessionMeta> {
        self.conn()
            .query_row(
                "SELECT agent, model, project, start_time, vault_path FROM sessions WHERE id = ?1",
                [session_id],
                |row| {
                    let start_time: String = row.get(3)?;
                    let date = start_time.get(..10).unwrap_or("").to_string();
                    Ok(SessionMeta {
                        agent: row.get(0)?,
                        model: row.get(1)?,
                        project: row.get(2)?,
                        date,
                        vault_path: row.get(4)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    SecallError::SessionNotFound(session_id.to_string())
                }
                _ => SecallError::Database(e),
            })
    }
}

// SearchRepo impl for Database — FTS index + search
impl SearchRepo for Database {
    fn insert_fts(
        &self,
        tokenized_content: &str,
        session_id: &str,
        turn_index: u32,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            // FTS5 컬럼명 turn_id는 유지 (스키마 변경 최소화). 저장값은 실제 turn_index.
            "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![tokenized_content, session_id, turn_index as i64],
        )?;
        Ok(())
    }

    fn search_fts(
        &self,
        tokenized_query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> crate::error::Result<Vec<FtsRow>> {
        let since_str = filters.since.map(|dt| dt.to_rfc3339());
        let until_str = filters.until.map(|dt| dt.to_rfc3339());

        let mut stmt = self.conn().prepare(
            "SELECT turns_fts.session_id, turns_fts.turn_id, turns_fts.content, bm25(turns_fts) as score
             FROM turns_fts
             JOIN sessions ON turns_fts.session_id = sessions.id
             WHERE turns_fts.content MATCH ?1
               AND (?2 IS NULL OR sessions.start_time >= ?2)
               AND (?3 IS NULL OR sessions.start_time < ?3)
             ORDER BY score
             LIMIT ?4",
        )?;

        let rows = stmt.query_map(
            rusqlite::params![tokenized_query, since_str, until_str, limit as i64],
            |row| {
                Ok(FtsRow {
                    session_id: row.get(0)?,
                    turn_index: row.get::<_, i64>(1)? as u32,
                    content: row.get(2)?,
                    score: -row.get::<_, f64>(3)?,
                })
            },
        )?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(SecallError::Database)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use crate::search::tokenizer::LinderaKoTokenizer;
    use crate::store::db::Database;
    use chrono::{TimeZone, Utc};

    fn make_session(id: &str, project: &str, content: &str) -> Session {
        Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: Some("test-model".to_string()),
            project: Some(project.to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc.with_ymd_and_hms(2026, 4, 5, 0, 0, 0).unwrap(),
            end_time: None,
            turns: vec![Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: content.to_string(),
                actions: Vec::new(),
                tokens: None,
                thinking: None,
                is_sidechain: false,
            }],
            total_tokens: TokenUsage::default(),
        }
    }

    #[test]
    fn test_index_and_search() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let session = make_session("s1", "myproject", "아키텍처 설계 방법");
        indexer.index_session(&db, &session).unwrap();

        let results = indexer
            .search(&db, "아키텍처", 10, &SearchFilters::default())
            .unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let results = indexer
            .search(&db, "", 10, &SearchFilters::default())
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_no_match_returns_empty() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let session = make_session("s2", "proj", "hello world test");
        indexer.index_session(&db, &session).unwrap();

        let results = indexer
            .search(&db, "완전히없는단어xyz", 10, &SearchFilters::default())
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_score_normalization() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let session1 = make_session("s3", "proj", "rust workspace 초기화 방법");
        let session2 = make_session("s4", "proj", "rust 설계 패턴");
        indexer.index_session(&db, &session1).unwrap();
        indexer.index_session(&db, &session2).unwrap();

        let results = indexer
            .search(&db, "rust", 10, &SearchFilters::default())
            .unwrap();
        assert!(!results.is_empty());
        // Max score should be 1.0
        let max = results
            .iter()
            .map(|r| r.score)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max - 1.0).abs() < 0.01);
    }

    fn make_multi_turn_session(id: &str, turns: Vec<(&str, &str)>) -> Session {
        Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: Some("test-model".to_string()),
            project: Some("proj".to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc.with_ymd_and_hms(2026, 4, 5, 0, 0, 0).unwrap(),
            end_time: None,
            turns: turns
                .into_iter()
                .enumerate()
                .map(|(i, (_, content))| Turn {
                    index: i as u32,
                    role: Role::User,
                    timestamp: None,
                    content: content.to_string(),
                    actions: Vec::new(),
                    tokens: None,
                    thinking: None,
                    is_sidechain: false,
                })
                .collect(),
            total_tokens: TokenUsage::default(),
        }
    }

    #[test]
    fn test_turn_index_not_rowid() {
        // 두 세션을 인덱싱하여 rowid가 turn_index와 다른 상황 재현
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        // Session 1: 3 turns (turn_index 0, 1, 2), rowid 1, 2, 3
        let session1 = make_multi_turn_session(
            "s-first",
            vec![
                ("", "첫번째 세션 첫턴"),
                ("", "첫번째 세션 두번째턴"),
                ("", "아키텍처 설계"),
            ],
        );
        indexer.index_session(&db, &session1).unwrap();

        // Session 2: 2 turns (turn_index 0, 1), rowid 4, 5
        let session2 = make_multi_turn_session(
            "s-second",
            vec![("", "두번째 세션 아키텍처"), ("", "두번째 세션 마지막")],
        );
        indexer.index_session(&db, &session2).unwrap();

        // "아키텍처"로 검색
        let results = indexer
            .search(&db, "아키텍처", 10, &SearchFilters::default())
            .unwrap();
        assert!(!results.is_empty(), "검색 결과가 있어야 함");

        for r in &results {
            if r.session_id == "s-second" {
                assert_eq!(
                    r.turn_index, 0,
                    "session2의 turn_index는 0이어야 하나 rowid=4가 반환됨"
                );
            }
            if r.session_id == "s-first" {
                assert_eq!(r.turn_index, 2, "session1의 turn_index는 2이어야 함");
            }
        }
    }

    #[test]
    fn test_project_filter() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let session1 = make_session("s5", "projectA", "검색 기능 구현");
        let session2 = make_session("s6", "projectB", "검색 결과 표시");
        indexer.index_session(&db, &session1).unwrap();
        indexer.index_session(&db, &session2).unwrap();

        let filters = SearchFilters {
            project: Some("projectA".to_string()),
            ..Default::default()
        };
        let results = indexer.search(&db, "검색", 10, &filters).unwrap();
        assert!(results
            .iter()
            .all(|r| r.metadata.project.as_deref() == Some("projectA")));
    }
}
