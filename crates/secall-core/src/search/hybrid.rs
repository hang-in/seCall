use std::collections::HashMap;

use chrono::Utc;

use super::bm25::{Bm25Indexer, GraphFilter, IndexStats, SearchFilters, SearchResult};
use super::vector::VectorIndexer;
use crate::ingest::Session;
use crate::store::db::Database;

const RRF_K: f64 = 60.0;

pub fn reciprocal_rank_fusion(
    bm25_results: &[SearchResult],
    vector_results: &[SearchResult],
    k: f64,
) -> Vec<SearchResult> {
    // Key: (session_id, turn_index)
    let mut score_map: HashMap<(String, u32), f64> = HashMap::new();
    let mut result_map: HashMap<(String, u32), SearchResult> = HashMap::new();

    for (rank, r) in bm25_results.iter().enumerate() {
        let key = (r.session_id.clone(), r.turn_index);
        let rrf_score = 1.0 / (k + rank as f64 + 1.0);
        *score_map.entry(key.clone()).or_insert(0.0) += rrf_score;
        result_map.entry(key).or_insert_with(|| r.clone());
    }

    for (rank, r) in vector_results.iter().enumerate() {
        let key = (r.session_id.clone(), r.turn_index);
        let rrf_score = 1.0 / (k + rank as f64 + 1.0);
        *score_map.entry(key.clone()).or_insert(0.0) += rrf_score;
        // Update with vector scores
        let entry = result_map.entry(key).or_insert_with(|| r.clone());
        if r.vector_score.is_some() {
            entry.vector_score = r.vector_score;
        }
        if r.bm25_score.is_some() {
            entry.bm25_score = r.bm25_score;
        }
    }

    // Assign RRF scores
    let mut results: Vec<SearchResult> = result_map
        .into_iter()
        .map(|(key, mut r)| {
            r.score = score_map[&key];
            r
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Normalize 0.0–1.0
    if let Some(max) = results.first().map(|r| r.score) {
        if max > 0.0 {
            for r in results.iter_mut() {
                r.score /= max;
            }
        }
    }

    results
}

/// 세션당 최대 N개 턴만 유지하여 결과 다양성 확보.
/// 입력은 점수 내림차순 정렬되어 있어야 함.
pub(crate) fn diversify_by_session(
    results: Vec<SearchResult>,
    max_per_session: usize,
) -> Vec<SearchResult> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    results
        .into_iter()
        .filter(|r| {
            let count = counts.entry(r.session_id.clone()).or_insert(0);
            if *count < max_per_session {
                *count += 1;
                true
            } else {
                false
            }
        })
        .collect()
}

pub struct SearchEngine {
    bm25: Bm25Indexer,
    vector: Option<VectorIndexer>,
}

/// graph_filter가 설정된 경우 해당 노드와 연결된 세션 ID allowlist를 해석.
/// None이면 그대로 필터를 반환.
fn resolve_graph_filter<'a>(
    db: &Database,
    filters: &'a SearchFilters,
) -> std::borrow::Cow<'a, SearchFilters> {
    let Some(ref gf) = filters.graph_filter else {
        return std::borrow::Cow::Borrowed(filters);
    };

    let (node_prefix, label, relation_hint) = match gf {
        GraphFilter::Topic(t) => ("topic", t.as_str(), None),
        GraphFilter::File(f) => ("file", f.as_str(), Some("modifies_file")),
        GraphFilter::Issue(i) => ("issue", i.as_str(), Some("fixes_bug")),
    };

    let ids = db
        .resolve_graph_filter_to_session_ids(node_prefix, label, relation_hint)
        .unwrap_or_default();

    let mut resolved = filters.clone();
    resolved.session_ids_allowlist = Some(ids);
    std::borrow::Cow::Owned(resolved)
}

impl SearchEngine {
    pub fn new(bm25: Bm25Indexer, vector: Option<VectorIndexer>) -> Self {
        SearchEngine { bm25, vector }
    }

    pub async fn search(
        &self,
        db: &Database,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let resolved = resolve_graph_filter(db, filters);
        let filters = resolved.as_ref();

        let candidate_limit = limit * 3;

        let bm25_results = self.bm25.search(db, query, candidate_limit, filters)?;

        // 벡터 검색을 독립 실행 (BM25 결과로 범위 제한하지 않음)
        let vector_results = if let Some(vi) = &self.vector {
            vi.search(db, query, candidate_limit, filters, None)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if bm25_results.is_empty() && vector_results.is_empty() {
            return Ok(Vec::new());
        }

        let max_per = filters.max_per_session.unwrap_or(2);

        if vector_results.is_empty() {
            // BM25-only mode
            let mut results: Vec<_> = bm25_results.into_iter().collect();
            results = diversify_by_session(results, max_per);
            results.truncate(limit);
            return Ok(results);
        }

        if bm25_results.is_empty() {
            let mut results: Vec<_> = vector_results.into_iter().collect();
            results = diversify_by_session(results, max_per);
            results.truncate(limit);
            return Ok(results);
        }

        let mut combined = reciprocal_rank_fusion(&bm25_results, &vector_results, RRF_K);
        // 세션 다양성 적용 (기본값: 세션당 최대 2개)
        combined = diversify_by_session(combined, max_per);
        combined.truncate(limit);
        Ok(combined)
    }

    pub fn search_bm25(
        &self,
        db: &Database,
        query: &str,
        filters: &SearchFilters,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let resolved = resolve_graph_filter(db, filters);
        self.bm25.search(db, query, limit, resolved.as_ref())
    }

    pub async fn search_vector(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> anyhow::Result<Vec<SearchResult>> {
        match &self.vector {
            Some(vi) => {
                let resolved = resolve_graph_filter(db, filters);
                vi.search(db, query, limit, resolved.as_ref(), None).await
            }
            None => Ok(Vec::new()),
        }
    }

    /// Embed a query string without accessing the DB.
    /// Use this in async contexts where DB is behind a Mutex.
    pub async fn embed_query(&self, query: &str) -> anyhow::Result<Option<Vec<f32>>> {
        match &self.vector {
            Some(vi) => vi.embed_query(query).await.map(Some),
            None => Ok(None),
        }
    }

    /// Search vectors using a pre-computed embedding (sync DB call).
    /// Call `embed_query` first, then lock DB, then call this.
    pub fn search_with_embedding(
        &self,
        db: &Database,
        embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
    ) -> anyhow::Result<Vec<SearchResult>> {
        match &self.vector {
            Some(vi) => {
                let resolved = resolve_graph_filter(db, filters);
                vi.search_with_embedding(db, embedding, limit, resolved.as_ref(), None)
            }
            None => Ok(Vec::new()),
        }
    }

    pub async fn index_session(
        &self,
        db: &Database,
        session: &Session,
        tz: chrono_tz::Tz,
    ) -> anyhow::Result<IndexStats> {
        let mut stats = self.bm25.index_session(db, session)?;

        if let Some(vi) = &self.vector {
            let vec_stats = vi.index_session(db, session, tz).await?;
            stats.chunks_embedded += vec_stats.chunks_embedded;
            stats.errors += vec_stats.errors;
        }

        Ok(stats)
    }

    /// BM25 인덱싱만 수행 (동기, 트랜잭션 클로저 내에서 호출 가능)
    pub fn index_session_bm25(
        &self,
        db: &Database,
        session: &Session,
    ) -> anyhow::Result<IndexStats> {
        self.bm25.index_session(db, session)
    }

    /// 벡터 인덱싱만 수행 (비동기, 트랜잭션 밖에서 호출)
    pub async fn index_session_vectors(
        &self,
        db: &Database,
        session: &Session,
        tz: chrono_tz::Tz,
    ) -> anyhow::Result<IndexStats> {
        if let Some(ref v) = self.vector {
            v.index_session(db, session, tz).await
        } else {
            Ok(IndexStats::default())
        }
    }
}

/// Parse temporal filter strings into SearchFilters
pub fn parse_temporal_filter(input: &str) -> Option<SearchFilters> {
    let now = Utc::now();
    let lower = input.to_lowercase();

    match lower.as_str() {
        "today" => {
            let since = now.date_naive().and_hms_opt(0, 0, 0)?.and_utc();
            Some(SearchFilters {
                since: Some(since),
                ..Default::default()
            })
        }
        "yesterday" => {
            let today = now.date_naive().and_hms_opt(0, 0, 0)?.and_utc();
            let yesterday = today - chrono::Duration::days(1);
            Some(SearchFilters {
                since: Some(yesterday),
                until: Some(today),
                ..Default::default()
            })
        }
        "last week" | "this week" => {
            let since = now - chrono::Duration::days(7);
            Some(SearchFilters {
                since: Some(since),
                ..Default::default()
            })
        }
        s if s.starts_with("since ") => {
            let date_str = &s["since ".len()..];
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
            let since = date.and_hms_opt(0, 0, 0)?.and_utc();
            Some(SearchFilters {
                since: Some(since),
                ..Default::default()
            })
        }
        other => {
            tracing::warn!(
                input = other,
                "unrecognized temporal filter, ignoring --since value"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::bm25::SessionMeta;

    fn make_result(session_id: &str, turn: u32, score: f64) -> SearchResult {
        SearchResult {
            session_id: session_id.to_string(),
            turn_index: turn,
            score,
            bm25_score: Some(score),
            vector_score: None,
            snippet: String::new(),
            metadata: SessionMeta {
                agent: "claude-code".to_string(),
                model: None,
                project: None,
                date: "2026-04-05".to_string(),
                vault_path: None,
                session_type: "interactive".to_string(),
            },
        }
    }

    #[test]
    fn test_rrf_basic() {
        let bm25 = vec![
            make_result("A", 0, 1.0),
            make_result("B", 0, 0.8),
            make_result("C", 0, 0.5),
        ];
        let vector = vec![
            make_result("B", 0, 1.0),
            make_result("C", 0, 0.7),
            make_result("D", 0, 0.4),
        ];

        let combined = reciprocal_rank_fusion(&bm25, &vector, RRF_K);
        // B appears in both lists → should score highest
        assert!(!combined.is_empty());
        assert_eq!(combined[0].session_id, "B");
    }

    #[test]
    fn test_rrf_bm25_only() {
        let bm25 = vec![make_result("A", 0, 1.0), make_result("B", 0, 0.5)];
        let combined = reciprocal_rank_fusion(&bm25, &[], RRF_K);
        assert_eq!(combined.len(), 2);
        assert_eq!(combined[0].session_id, "A");
    }

    #[test]
    fn test_rrf_vector_only() {
        let vector = vec![make_result("X", 0, 1.0), make_result("Y", 0, 0.5)];
        let combined = reciprocal_rank_fusion(&[], &vector, RRF_K);
        assert_eq!(combined.len(), 2);
        assert_eq!(combined[0].session_id, "X");
    }

    #[test]
    fn test_rrf_empty_both() {
        let combined = reciprocal_rank_fusion(&[], &[], RRF_K);
        assert!(combined.is_empty());
    }

    #[test]
    fn test_rrf_score_normalization() {
        let bm25 = vec![make_result("A", 0, 0.9), make_result("B", 0, 0.5)];
        let combined = reciprocal_rank_fusion(&bm25, &[], RRF_K);
        let max_score = combined
            .iter()
            .map(|r| r.score)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((max_score - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_diversify_by_session() {
        let results = vec![
            make_result("A", 0, 1.0),
            make_result("A", 1, 0.9),
            make_result("A", 2, 0.8),
            make_result("B", 0, 0.7),
            make_result("C", 0, 0.6),
        ];
        let diversified = diversify_by_session(results, 2);
        // A는 2개만 유지, B와 C는 그대로
        assert_eq!(diversified.len(), 4);
        assert_eq!(
            diversified.iter().filter(|r| r.session_id == "A").count(),
            2
        );
        assert_eq!(
            diversified.iter().filter(|r| r.session_id == "B").count(),
            1
        );
        assert_eq!(
            diversified.iter().filter(|r| r.session_id == "C").count(),
            1
        );
    }

    #[test]
    fn test_diversify_max_1() {
        let results = vec![
            make_result("A", 0, 1.0),
            make_result("A", 1, 0.9),
            make_result("B", 0, 0.8),
        ];
        let diversified = diversify_by_session(results, 1);
        assert_eq!(diversified.len(), 2);
        assert_eq!(diversified[0].session_id, "A");
        assert_eq!(diversified[0].turn_index, 0); // 최고 점수 턴 유지
        assert_eq!(diversified[1].session_id, "B");
    }

    #[test]
    fn test_diversify_no_limit() {
        let results = vec![
            make_result("A", 0, 1.0),
            make_result("A", 1, 0.9),
            make_result("A", 2, 0.8),
        ];
        // max_per_session이 충분히 크면 모든 결과 유지
        let diversified = diversify_by_session(results, 100);
        assert_eq!(diversified.len(), 3);
    }

    #[test]
    fn test_temporal_filter_today() {
        let f = parse_temporal_filter("today");
        assert!(f.is_some());
        assert!(f.unwrap().since.is_some());
    }

    #[test]
    fn test_temporal_filter_yesterday() {
        let f = parse_temporal_filter("yesterday");
        assert!(f.is_some());
        let f = f.unwrap();
        assert!(f.since.is_some());
        assert!(f.until.is_some());
    }

    #[test]
    fn test_temporal_filter_since_date() {
        let f = parse_temporal_filter("since 2026-04-01");
        assert!(f.is_some());
        assert!(f.unwrap().since.is_some());
    }

    #[test]
    fn test_temporal_filter_unknown() {
        assert!(parse_temporal_filter("random text").is_none());
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    use crate::ingest::types::{AgentKind, Role, Session, TokenUsage, Turn};
    use crate::search::tokenizer::LinderaKoTokenizer;
    use crate::store::db::Database;
    use chrono::{TimeZone, Utc};

    fn make_session(id: &str, project: &str, content: &str) -> Session {
        Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
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
            session_type: "interactive".to_string(),
        }
    }

    #[test]
    fn test_bm25_only_search() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);

        let session = make_session("s1", "proj", "검색 기능 구현 방법");
        engine.bm25.index_session(&db, &session).unwrap();

        let results = engine
            .search_bm25(&db, "검색", &SearchFilters::default(), 5)
            .unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_project_filter_bm25() {
        let db = Database::open_memory().unwrap();
        let tok = LinderaKoTokenizer::new().unwrap();
        let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), None);

        let s1 = make_session("s1", "projectA", "테스트 코드 작성");
        let s2 = make_session("s2", "projectB", "테스트 실행 방법");
        engine.bm25.index_session(&db, &s1).unwrap();
        engine.bm25.index_session(&db, &s2).unwrap();

        let filters = SearchFilters {
            project: Some("projectA".to_string()),
            ..Default::default()
        };
        let results = engine.search_bm25(&db, "테스트", &filters, 10).unwrap();
        assert!(results
            .iter()
            .all(|r| r.metadata.project.as_deref() == Some("projectA")));
    }
}
