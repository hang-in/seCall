// Placeholder — will be fully defined after tokenizer builds
// to avoid circular compilation issues
use anyhow::Result;
use chrono::{DateTime, Utc};
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

/// 그래프 기반 사전 필터 — 검색 대상 세션을 특정 노드와 연결된 것으로 제한
#[derive(Debug, Clone)]
pub enum GraphFilter {
    /// 특정 토픽 노드와 연결된 세션 (e.g., "rust async")
    Topic(String),
    /// 특정 파일을 수정한 세션 (modifies_file 엣지)
    File(String),
    /// 특정 이슈를 수정한 세션 (fixes_bug 엣지, e.g., "#42")
    Issue(String),
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    /// 세션당 최대 결과 수 (None = 제한 없음)
    pub max_per_session: Option<usize>,
    /// 제외할 session_type 목록 (빈 Vec = 제외 없음)
    pub exclude_session_types: Vec<String>,
    /// 그래프 기반 사전 필터 (None = 필터 없음)
    pub graph_filter: Option<GraphFilter>,
    /// 검색 대상 세션 ID 허용 목록 — graph_filter 해석 결과가 여기에 삽입됨
    /// None = 제한 없음, Some([]) = 결과 없음 (일치하는 그래프 노드 없음)
    pub session_ids_allowlist: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionMeta {
    pub agent: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub date: String,
    pub vault_path: Option<String>,
    pub session_type: String,
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

        // 빈 allowlist → 결과 없음 (그래프 필터에 해당하는 세션이 없음)
        if let Some(ref ids) = filters.session_ids_allowlist {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
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
            session_type: "interactive".to_string(),
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
            session_type: "interactive".to_string(),
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

    #[test]
    fn test_empty_allowlist_returns_empty() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let indexer = Bm25Indexer::new(Box::new(tok));

        let session = make_session("s7", "proj", "rust 검색 테스트");
        indexer.index_session(&db, &session).unwrap();

        // 빈 allowlist → 결과 없음
        let filters = SearchFilters {
            session_ids_allowlist: Some(vec![]),
            ..Default::default()
        };
        let results = indexer.search(&db, "rust", 10, &filters).unwrap();
        assert!(results.is_empty(), "빈 allowlist는 결과 없음이어야 함");
    }
}
