use std::collections::HashMap;

use serde::Serialize;

use crate::error::Result;

use super::db::Database;

#[derive(Debug, Default)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes_by_type: HashMap<String, usize>,
    pub edges_by_relation: HashMap<String, usize>,
}

/// 검색 결과 세션과 그래프로 연결된 관련 세션 정보
#[derive(Debug, Clone, Serialize)]
pub struct RelatedSession {
    /// 관련 세션 ID
    pub session_id: String,
    /// 연결 관계 타입 (e.g., "same_project", "fixes_bug", "discusses_topic")
    pub relation: String,
    /// 탐색 깊이 (1 = 직접 연결, 2 = 2홉, 3 = 3홉)
    pub hop_count: usize,
    /// 에이전트 이름
    pub agent: String,
    /// 프로젝트 이름
    pub project: Option<String>,
    /// 세션 날짜 (YYYY-MM-DD)
    pub date: String,
    /// 세션 요약 (첫 번째 사용자 발화 기반)
    pub summary: Option<String>,
}

impl Database {
    /// 노드 upsert (INSERT OR REPLACE)
    pub fn upsert_graph_node(
        &self,
        id: &str,
        node_type: &str,
        label: &str,
        meta: Option<&str>,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO graph_nodes(id, type, label, meta) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, node_type, label, meta],
        )?;
        Ok(())
    }

    /// 엣지 upsert (INSERT OR IGNORE — 중복 무시)
    /// 반환값: 실제 삽입된 행 수 (0 = 중복으로 무시됨, 1 = 삽입됨)
    pub fn upsert_graph_edge(
        &self,
        source: &str,
        target: &str,
        relation: &str,
        confidence: &str,
        weight: f64,
    ) -> Result<usize> {
        let rows = self.conn().execute(
            "INSERT OR IGNORE INTO graph_edges(source, target, relation, confidence, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![source, target, relation, confidence, weight],
        )?;
        Ok(rows)
    }

    /// 노드의 이웃 조회 (양방향)
    /// 반환: Vec<(neighbor_id, relation, direction)>  direction: "out" | "in"
    pub fn get_neighbors(&self, node_id: &str) -> Result<Vec<(String, String, String)>> {
        let mut results = Vec::new();

        // 나가는 엣지 (source = node_id)
        let mut stmt = self
            .conn()
            .prepare("SELECT target, relation FROM graph_edges WHERE source = ?1")?;
        let out_rows = stmt.query_map([node_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in out_rows.filter_map(|r| r.ok()) {
            results.push((row.0, row.1, "out".to_string()));
        }

        // 들어오는 엣지 (target = node_id)
        let mut stmt = self
            .conn()
            .prepare("SELECT source, relation FROM graph_edges WHERE target = ?1")?;
        let in_rows = stmt.query_map([node_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in in_rows.filter_map(|r| r.ok()) {
            results.push((row.0, row.1, "in".to_string()));
        }

        Ok(results)
    }

    /// 그래프 통계
    pub fn graph_stats(&self) -> Result<GraphStats> {
        // graph_nodes 테이블이 없으면 빈 stats 반환
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_nodes'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(GraphStats::default());
        }

        let node_count: i64 =
            self.conn()
                .query_row("SELECT COUNT(*) FROM graph_nodes", [], |r| r.get(0))?;
        let edge_count: i64 =
            self.conn()
                .query_row("SELECT COUNT(*) FROM graph_edges", [], |r| r.get(0))?;

        let mut nodes_by_type: HashMap<String, usize> = HashMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT type, COUNT(*) FROM graph_nodes GROUP BY type")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows.filter_map(|r| r.ok()) {
            nodes_by_type.insert(row.0, row.1 as usize);
        }

        let mut edges_by_relation: HashMap<String, usize> = HashMap::new();
        let mut stmt = self
            .conn()
            .prepare("SELECT relation, COUNT(*) FROM graph_edges GROUP BY relation")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows.filter_map(|r| r.ok()) {
            edges_by_relation.insert(row.0, row.1 as usize);
        }

        Ok(GraphStats {
            node_count: node_count as usize,
            edge_count: edge_count as usize,
            nodes_by_type,
            edges_by_relation,
        })
    }

    /// 전체 노드 목록 (type 필터 선택)
    /// 반환: Vec<(id, type, label)>
    pub fn list_graph_nodes(
        &self,
        node_type: Option<&str>,
    ) -> Result<Vec<(String, String, String)>> {
        if let Some(t) = node_type {
            let mut stmt = self
                .conn()
                .prepare("SELECT id, type, label FROM graph_nodes WHERE type = ?1")?;
            let rows: Vec<_> = stmt
                .query_map([t], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        } else {
            let mut stmt = self
                .conn()
                .prepare("SELECT id, type, label FROM graph_nodes")?;
            let rows: Vec<_> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(rows)
        }
    }

    /// 그래프 전체 초기화 (--force 용)
    pub fn clear_graph(&self) -> Result<()> {
        self.conn()
            .execute_batch("DELETE FROM graph_edges; DELETE FROM graph_nodes;")?;
        Ok(())
    }

    /// 특정 세션과 관련된 그래프 데이터 삭제 (증분 재빌드 용)
    pub fn delete_graph_for_session(&self, session_id: &str) -> Result<()> {
        let node_id = format!("session:{}", session_id);
        // 해당 세션 노드가 source/target인 엣지 삭제
        self.conn().execute(
            "DELETE FROM graph_edges WHERE source = ?1 OR target = ?1",
            rusqlite::params![node_id],
        )?;
        // 세션 노드 삭제
        self.conn().execute(
            "DELETE FROM graph_nodes WHERE id = ?1",
            rusqlite::params![node_id],
        )?;
        Ok(())
    }

    /// 이미 그래프에 포함된 세션 ID 목록
    pub fn list_graphed_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id FROM graph_nodes WHERE type = 'session'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        // "session:{id}" → "{id}" 변환
        let ids = rows
            .filter_map(|r| r.ok())
            .filter_map(|s| s.strip_prefix("session:").map(|id| id.to_string()))
            .collect();
        Ok(ids)
    }

    /// 검색 결과 세션 ID 목록을 기반으로 그래프 BFS 탐색하여 관련 세션 반환.
    ///
    /// - `seed_session_ids`: 검색 결과에서 나온 세션 ID 슬라이스
    /// - `max_hops`: 최대 탐색 깊이 (1~3, 기본 2)
    /// - `limit`: 반환할 최대 관련 세션 수
    ///
    /// 반환값은 hop_count 오름차순 → 동일 hop에서 relation 알파벳 순으로 정렬.
    /// seed 세션 자신은 결과에서 제외.
    pub fn get_related_sessions(
        &self,
        seed_session_ids: &[&str],
        max_hops: usize,
        limit: usize,
    ) -> Result<Vec<RelatedSession>> {
        if seed_session_ids.is_empty() || limit == 0 {
            return Ok(vec![]);
        }
        let max_hops = max_hops.min(3).max(1);

        // graph_nodes 테이블이 없으면 빈 결과 반환
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_nodes'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(vec![]);
        }

        let seed_set: std::collections::HashSet<String> = seed_session_ids
            .iter()
            .map(|id| format!("session:{}", id))
            .collect();

        // BFS: node_id → (relation, hop_count)
        // 같은 노드에 여러 경로 존재 시 최단 hop만 기록
        let mut found: HashMap<String, (String, usize)> = HashMap::new();
        let mut frontier: Vec<String> = seed_set.iter().cloned().collect();

        for hop in 1..=max_hops {
            if frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            for node in &frontier {
                // 나가는 엣지
                let mut stmt = self
                    .conn()
                    .prepare("SELECT target, relation FROM graph_edges WHERE source = ?1")?;
                let out: Vec<(String, String)> = stmt
                    .query_map([node], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                // 들어오는 엣지
                let mut stmt = self
                    .conn()
                    .prepare("SELECT source, relation FROM graph_edges WHERE target = ?1")?;
                let inc: Vec<(String, String)> = stmt
                    .query_map([node], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                for (neighbor, relation) in out.into_iter().chain(inc) {
                    // session 노드만, seed에 포함되지 않은 것만, 아직 미발견인 것만
                    if neighbor.starts_with("session:")
                        && !seed_set.contains(&neighbor)
                        && !found.contains_key(&neighbor)
                    {
                        found.insert(neighbor.clone(), (relation, hop));
                        next_frontier.push(neighbor);
                    }
                }
            }
            frontier = next_frontier;
        }

        if found.is_empty() {
            return Ok(vec![]);
        }

        // 세션 메타 일괄 조회
        let session_ids: Vec<String> = found
            .keys()
            .filter_map(|k| k.strip_prefix("session:").map(|s| s.to_string()))
            .take(200)
            .collect();

        let placeholders: String = (1..=session_ids.len())
            .map(|i| format!("?{}", i))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, agent, project, DATE(start_time) as date, summary \
             FROM sessions WHERE id IN ({})",
            placeholders
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = session_ids
            .iter()
            .map(|s| s as &dyn rusqlite::types::ToSql)
            .collect();

        let meta_map: HashMap<String, (String, Option<String>, String, Option<String>)> = stmt
            .query_map(params.as_slice(), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    (
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                    ),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        // RelatedSession 목록 생성
        let mut results: Vec<RelatedSession> = found
            .iter()
            .filter_map(|(node_id, (relation, hop))| {
                let sid = node_id.strip_prefix("session:")?;
                let (agent, project, date, summary) = meta_map.get(sid)?.clone();
                Some(RelatedSession {
                    session_id: sid.to_string(),
                    relation: relation.clone(),
                    hop_count: *hop,
                    agent,
                    project,
                    date,
                    summary,
                })
            })
            .collect();

        // hop_count 오름차순, 동일 hop은 relation 알파벳 순
        results.sort_by(|a, b| {
            a.hop_count
                .cmp(&b.hop_count)
                .then(a.relation.cmp(&b.relation))
        });
        results.truncate(limit);

        Ok(results)
    }

    /// 그래프 필터 조건에 해당하는 세션 ID 목록 반환.
    /// topic/file/issue 노드와 연결된 세션들을 찾아 ID만 반환.
    /// 결과가 없으면 빈 Vec (→ SearchFilters.session_ids_allowlist = Some([]) → 검색 결과 0개).
    pub fn resolve_graph_filter_to_session_ids(
        &self,
        node_prefix: &str,
        label_query: &str,
        relation_filter: Option<&str>,
    ) -> Result<Vec<String>> {
        // graph_nodes 테이블이 없으면 빈 결과
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_nodes'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(vec![]);
        }

        // label 부분 일치로 대상 노드 찾기 (LIKE, case-insensitive)
        let node_id_pattern = format!("{}:%", node_prefix);
        let label_pattern = format!("%{}%", label_query.to_lowercase());

        let mut target_nodes: Vec<String> = {
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM graph_nodes WHERE id LIKE ?1 AND lower(label) LIKE ?2")?;
            let rows = stmt
                .query_map(rusqlite::params![node_id_pattern, label_pattern], |r| {
                    r.get::<_, String>(0)
                })?
                .filter_map(|r| r.ok())
                .collect();
            rows
        };

        // fallback: id 자체에서 일치 검색
        if target_nodes.is_empty() {
            let id_pattern = format!("{}:%{}%", node_prefix, label_query.to_lowercase());
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM graph_nodes WHERE lower(id) LIKE ?1")?;
            target_nodes = stmt
                .query_map([&id_pattern], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
        }

        if target_nodes.is_empty() {
            return Ok(vec![]);
        }

        // 해당 노드와 연결된 세션 노드 수집
        let mut session_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        for target_node in &target_nodes {
            // session → target (나가는 방향)
            let rel_clause = relation_filter
                .map(|r| format!("AND relation = '{}'", r))
                .unwrap_or_default();
            let sql = format!(
                "SELECT source FROM graph_edges WHERE target = ?1 AND source LIKE 'session:%' {}",
                rel_clause
            );
            let mut stmt = self.conn().prepare(&sql)?;
            let ids: Vec<String> = stmt
                .query_map([target_node], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            for id in ids {
                if let Some(sid) = id.strip_prefix("session:") {
                    session_ids.insert(sid.to_string());
                }
            }

            // target → session (들어오는 방향, 역방향 엣지)
            let sql = format!(
                "SELECT target FROM graph_edges WHERE source = ?1 AND target LIKE 'session:%' {}",
                rel_clause
            );
            let mut stmt = self.conn().prepare(&sql)?;
            let ids: Vec<String> = stmt
                .query_map([target_node], |r| r.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            for id in ids {
                if let Some(sid) = id.strip_prefix("session:") {
                    session_ids.insert(sid.to_string());
                }
            }
        }

        Ok(session_ids.into_iter().collect())
    }

    /// 특정 relation 타입의 엣지를 전체 삭제.
    /// 증분 빌드에서 same_project/same_day를 전체 재계산할 때 사용.
    pub fn delete_relation_edges(&self, relations: &[&str]) -> Result<usize> {
        if relations.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = (1..=relations.len()).map(|i| format!("?{}", i)).collect();
        let sql = format!(
            "DELETE FROM graph_edges WHERE relation IN ({})",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = relations
            .iter()
            .map(|r| r as &dyn rusqlite::types::ToSql)
            .collect();
        let deleted = self.conn().execute(&sql, params.as_slice())?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Database;

    #[test]
    fn test_graph_upsert_and_stats() {
        let db = Database::open_memory().unwrap();

        // 노드 삽입
        db.upsert_graph_node("session:abc123", "session", "Session ABC", None)
            .unwrap();
        db.upsert_graph_node("project:tunaflow", "project", "tunaflow", None)
            .unwrap();
        db.upsert_graph_node("agent:claude-code", "agent", "claude-code", None)
            .unwrap();

        // 엣지 삽입
        db.upsert_graph_edge(
            "session:abc123",
            "project:tunaflow",
            "belongs_to",
            "EXTRACTED",
            1.0,
        )
        .unwrap();
        db.upsert_graph_edge(
            "session:abc123",
            "agent:claude-code",
            "by_agent",
            "EXTRACTED",
            1.0,
        )
        .unwrap();

        // 중복 엣지 — INSERT OR IGNORE이므로 무시
        db.upsert_graph_edge(
            "session:abc123",
            "project:tunaflow",
            "belongs_to",
            "EXTRACTED",
            1.0,
        )
        .unwrap();

        let stats = db.graph_stats().unwrap();
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 2);
        assert_eq!(stats.nodes_by_type.get("session"), Some(&1));
        assert_eq!(stats.nodes_by_type.get("project"), Some(&1));
        assert_eq!(stats.edges_by_relation.get("belongs_to"), Some(&1));
    }

    #[test]
    fn test_delete_relation_edges() {
        let db = Database::open_memory().unwrap();

        // same_project 엣지 3개 삽입
        db.upsert_graph_node("session:s1", "session", "S1", None)
            .unwrap();
        db.upsert_graph_node("session:s2", "session", "S2", None)
            .unwrap();
        db.upsert_graph_node("session:s3", "session", "S3", None)
            .unwrap();
        db.upsert_graph_node("project:p1", "project", "P1", None)
            .unwrap();

        db.upsert_graph_edge("session:s1", "session:s2", "same_project", "EXTRACTED", 1.0)
            .unwrap();
        db.upsert_graph_edge("session:s2", "session:s3", "same_project", "EXTRACTED", 1.0)
            .unwrap();
        db.upsert_graph_edge("session:s1", "session:s3", "same_project", "EXTRACTED", 1.0)
            .unwrap();
        // belongs_to 엣지 2개 삽입
        db.upsert_graph_edge("session:s1", "project:p1", "belongs_to", "EXTRACTED", 1.0)
            .unwrap();
        db.upsert_graph_edge("session:s2", "project:p1", "belongs_to", "EXTRACTED", 1.0)
            .unwrap();

        // same_project만 삭제
        let deleted = db.delete_relation_edges(&["same_project"]).unwrap();
        assert_eq!(deleted, 3);

        // same_project 0개, belongs_to 2개 확인
        let stats = db.graph_stats().unwrap();
        assert_eq!(stats.edges_by_relation.get("same_project"), None);
        assert_eq!(stats.edges_by_relation.get("belongs_to"), Some(&2));
    }

    #[test]
    fn test_graph_neighbors() {
        let db = Database::open_memory().unwrap();

        db.upsert_graph_node("session:s1", "session", "S1", None)
            .unwrap();
        db.upsert_graph_node("project:p1", "project", "P1", None)
            .unwrap();
        db.upsert_graph_node("tool:Edit", "tool", "Edit", None)
            .unwrap();

        db.upsert_graph_edge("session:s1", "project:p1", "belongs_to", "EXTRACTED", 1.0)
            .unwrap();
        db.upsert_graph_edge("session:s1", "tool:Edit", "uses_tool", "EXTRACTED", 1.0)
            .unwrap();

        // session:s1 이웃 — 2개 (나가는 방향)
        let neighbors = db.get_neighbors("session:s1").unwrap();
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors
            .iter()
            .any(|(id, _, d)| id == "project:p1" && d == "out"));
        assert!(neighbors
            .iter()
            .any(|(id, _, d)| id == "tool:Edit" && d == "out"));

        // project:p1 이웃 — 1개 (들어오는 방향)
        let nb = db.get_neighbors("project:p1").unwrap();
        assert_eq!(nb.len(), 1);
        assert_eq!(nb[0].2, "in");
    }

    #[test]
    fn test_get_related_sessions_empty_seed() {
        let db = Database::open_memory().unwrap();
        let related = db.get_related_sessions(&[], 2, 5).unwrap();
        assert!(related.is_empty());
    }

    #[test]
    fn test_get_related_sessions_basic() {
        let db = Database::open_memory().unwrap();

        // 세션 노드 3개
        db.upsert_graph_node("session:s1", "session", "S1", None)
            .unwrap();
        db.upsert_graph_node("session:s2", "session", "S2", None)
            .unwrap();
        db.upsert_graph_node("session:s3", "session", "S3", None)
            .unwrap();
        db.upsert_graph_node("project:p1", "project", "P1", None)
            .unwrap();

        // s1 → p1 (same_project)
        db.upsert_graph_edge("session:s1", "project:p1", "same_project", "RULE", 1.0)
            .unwrap();
        // s2 → p1 (same_project)
        db.upsert_graph_edge("session:s2", "project:p1", "same_project", "RULE", 1.0)
            .unwrap();
        // s3 → p1 (same_project)
        db.upsert_graph_edge("session:s3", "project:p1", "same_project", "RULE", 1.0)
            .unwrap();

        // s1을 시드로 관련 세션 탐색 — 그래프에 세션 메타가 없으므로 found는 채워지지만 meta_map이 비어서 결과는 0
        // (실제 DB에 sessions 행이 없으므로 RelatedSession 변환 단계에서 필터됨)
        let related = db.get_related_sessions(&["s1"], 2, 5).unwrap();
        // sessions 테이블에 데이터가 없으므로 meta_map이 비어 결과 0
        assert_eq!(related.len(), 0);
    }
}
