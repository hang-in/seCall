use std::collections::HashMap;

use rusqlite::OptionalExtension;
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
    /// Adamic-Adar 관련도 점수 (시드와 공유하는 희소 엔티티일수록 높음). 랭킹 기준.
    pub score: f32,
}

/// `graph insights` 결과 — 그래프 기반 발견/큐레이션 리포트 (검색 아님).
#[derive(Debug, Default, Serialize)]
pub struct GraphInsights {
    /// 희소 엔티티(file/issue/tech)를 공유해 강하게 연결된 세션 쌍 (AA 내림차순).
    pub surprising: Vec<SurprisingPair>,
    /// 지식 공백 진단 (고립 세션 / 싱글턴 아티팩트 / degree 분포).
    pub gaps: KnowledgeGaps,
}

/// 공유 희소 엔티티로 연결된 세션 쌍. `session_a < session_b` 로 정규화.
/// **cross-project 만** 수록 — 두 세션의 project 가 서로 다를 때(진짜 교차 연결).
#[derive(Debug, Clone, Serialize)]
pub struct SurprisingPair {
    /// 세션 ID (`session:` 프리픽스 제거, 사전순 앞).
    pub session_a: String,
    /// 세션 ID (사전순 뒤).
    pub session_b: String,
    /// session_a 의 project (cross-project 판정·표시용).
    pub project_a: Option<String>,
    /// session_b 의 project.
    pub project_b: Option<String>,
    /// Adamic-Adar 관련도 = 공유 엔티티 Σ 1/ln(deg). 높을수록 희소 공유.
    pub score: f32,
    /// 공유 엔티티 근거 (node_id, 예: "issue:42", "file:src/foo.rs"). 기여도 내림차순.
    pub shared: Vec<String>,
}

/// 지식 공백 진단 집계.
#[derive(Debug, Default, Serialize)]
pub struct KnowledgeGaps {
    /// 엔티티 엣지가 하나도 없는 고립 세션 노드 수 (semantic 추출 누락/빈 세션).
    pub isolated_session_count: usize,
    /// 고립 세션 예시 (session_id, 최대 예시 개수).
    pub isolated_session_examples: Vec<String>,
    /// deg=1 (한 세션만 참조) file 노드 수 — 고아 파일 지식.
    pub singleton_file_count: usize,
    /// deg=1 issue 노드 수 — 한 세션에만 등장한 이슈.
    pub singleton_issue_count: usize,
    /// 싱글턴 아티팩트 예시 (node_id, issue 우선, 최대 예시 개수).
    pub singleton_examples: Vec<String>,
    /// `sessions` 테이블 총 세션 수.
    pub sessions_total: usize,
    /// 그래프에 session 노드로 편입된 세션 수.
    pub sessions_in_graph: usize,
    /// 그래프에 아예 편입되지 않은 세션 수 (semantic 추출 대상이지만 미처리 — 최대 knowledge gap).
    pub sessions_missing_from_graph: usize,
    /// 엣지≥1 세션의 degree(연결 엔티티 수) 최소값.
    pub session_degree_min: usize,
    /// 엣지≥1 세션의 degree 중앙값.
    pub session_degree_median: usize,
    /// 엣지≥1 세션의 degree 최대값.
    pub session_degree_max: usize,
}

/// `surprising_pairs` 누적기 값: (AA score 합, [(공유 엔티티 node_id, 기여 weight)]).
type PairAccum = (f32, Vec<(String, f32)>);

/// `?start, ?start+1, ...` SQL 플레이스홀더 생성 (동적 IN 절용).
fn sql_placeholders(start: usize, count: usize) -> String {
    (start..start + count)
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ")
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

    /// 노드의 type, label, meta 조회
    pub fn get_node_metadata(
        &self,
        node_id: &str,
    ) -> Result<Option<(String, String, Option<String>)>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT type, label, meta FROM graph_nodes WHERE id = ?1")?;
        let result = stmt
            .query_row([node_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .optional()?;
        Ok(result)
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
    /// 반환값은 Adamic-Adar 관련도 점수 내림차순(공유 엔티티 강도) → 동점은 hop → session_id 순.
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
        let max_hops = max_hops.clamp(1, 3);

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

        // Adamic-Adar 관련도 (시드와 공유하는 희소 엔티티일수록 강함) — 재랭킹 기준.
        let cand_nodes: Vec<String> = found.keys().cloned().collect();
        let aa_scores = self.adamic_adar_scores(&seed_set, &cand_nodes)?;

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
                    score: aa_scores.get(node_id).copied().unwrap_or(0.0),
                })
            })
            .collect();

        // Adamic-Adar 점수 내림차순(공유 엔티티 강도) → 동점은 hop 오름차 → session_id (결정성)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.hop_count.cmp(&b.hop_count))
                .then(a.session_id.cmp(&b.session_id))
        });
        results.truncate(limit);

        Ok(results)
    }

    /// Adamic-Adar 관련도: 시드 세션과 후보 세션이 공유하는 엔티티(project/tool/tech/topic/file/issue)
    /// 기반. 각 공유 엔티티 z 의 기여 = 1/ln(deg(z)) — 희소 엔티티(특정 file/issue)는 강하게,
    /// 흔한 허브는 약하게 반영. `agent:` 는 준-보편 허브라 제외. deg = z 에 연결된 distinct 세션 수.
    fn adamic_adar_scores(
        &self,
        seed_set: &std::collections::HashSet<String>,
        candidate_nodes: &[String],
    ) -> Result<HashMap<String, f32>> {
        // 배치 3쿼리로 N+1 회피: (1) 시드 엔티티 → (2) degree/weight → (3) 후보 연결 누적.
        if candidate_nodes.is_empty() {
            return Ok(HashMap::new());
        }

        // 1. 시드 엔티티 (session/agent 제외) — 1 쿼리
        let seeds: Vec<&String> = seed_set.iter().collect();
        let seed_entities: Vec<String> = {
            let sql = format!(
                "SELECT DISTINCT target FROM graph_edges \
                 WHERE source IN ({}) \
                   AND target NOT LIKE 'session:%' AND target NOT LIKE 'agent:%'",
                sql_placeholders(1, seeds.len())
            );
            let mut stmt = self.conn().prepare(&sql)?;
            let params: Vec<&dyn rusqlite::types::ToSql> = seeds
                .iter()
                .map(|s| *s as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt.query_map(params.as_slice(), |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        if seed_entities.is_empty() {
            return Ok(HashMap::new());
        }

        // 2. 시드 엔티티별 degree(연결된 distinct 세션 수) → weight (deg>=2 만) — 1 GROUP BY 쿼리
        let ent_weight: HashMap<String, f64> = {
            let sql = format!(
                "SELECT target, COUNT(DISTINCT source) FROM graph_edges \
                 WHERE target IN ({}) AND source LIKE 'session:%' GROUP BY target",
                sql_placeholders(1, seed_entities.len())
            );
            let mut stmt = self.conn().prepare(&sql)?;
            let params: Vec<&dyn rusqlite::types::ToSql> = seed_entities
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let rows = stmt.query_map(params.as_slice(), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?;
            rows.filter_map(|r| r.ok())
                .filter(|(_, deg)| *deg >= 2)
                .map(|(z, deg)| (z, 1.0 / (deg as f64).ln()))
                .collect()
        };
        if ent_weight.is_empty() {
            return Ok(HashMap::new());
        }

        // 3. 후보 ↔ 가중 시드엔티티 연결 — 1 쿼리, weight 누적
        let weighted: Vec<&String> = ent_weight.keys().collect();
        let sql = format!(
            "SELECT source, target FROM graph_edges \
             WHERE source IN ({}) AND target IN ({})",
            sql_placeholders(1, candidate_nodes.len()),
            sql_placeholders(candidate_nodes.len() + 1, weighted.len())
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::new();
        for c in candidate_nodes {
            params.push(c as &dyn rusqlite::types::ToSql);
        }
        for z in &weighted {
            params.push(*z as &dyn rusqlite::types::ToSql);
        }
        let mut scores: HashMap<String, f32> = HashMap::new();
        let pairs = stmt.query_map(params.as_slice(), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for (cand, ent) in pairs.filter_map(|r| r.ok()) {
            if let Some(w) = ent_weight.get(&ent) {
                *scores.entry(cand).or_insert(0.0) += *w as f32;
            }
        }
        Ok(scores)
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

    /// 그래프 발견/큐레이션 리포트: surprising connections + knowledge gaps.
    ///
    /// - **surprising**: 희소 엔티티(file/issue/tech, `2 ≤ deg ≤ deg_cap`)를 공유하는 세션 쌍을
    ///   Adamic-Adar(`Σ 1/ln(deg)`)로 랭킹. 흔한 허브는 deg_cap 으로 배제. topic 은 라벨 파편화가
    ///   심해 1차에서 제외.
    /// - **gaps**: 고립 세션(엣지 0) + 싱글턴 file/issue(deg 1) + 세션 degree 분포.
    ///
    /// 세션-세션 직접 엣지가 없는 스키마이므로 "직접 엣지 없음"은 자동 충족 — 공유 엔티티가 유일 신호.
    pub fn graph_insights(&self, top_n: usize, deg_cap: usize) -> Result<GraphInsights> {
        // graph_edges 테이블이 없으면 빈 결과
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='graph_edges'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(GraphInsights::default());
        }

        let surprising = self.surprising_pairs(top_n, deg_cap)?;
        let gaps = self.knowledge_gaps()?;
        Ok(GraphInsights { surprising, gaps })
    }

    /// 희소 엔티티 공유 세션 쌍을 AA 로 랭킹 (`graph_insights` 내부).
    ///
    /// CTE 로 후보 엔티티(`2 ≤ deg ≤ deg_cap`, file/issue/tech)를 먼저 좁혀 self-join 폭발을
    /// 방지한다. 엔티티당 쌍 수는 최대 `C(deg_cap, 2)` 로 상한. 한 쿼리로 (a, b, ent, deg) 튜플을
    /// 받아 메모리에서 (a,b) 별 weight 누적.
    fn surprising_pairs(&self, top_n: usize, deg_cap: usize) -> Result<Vec<SurprisingPair>> {
        if top_n == 0 {
            return Ok(vec![]);
        }
        let sql = "\
            WITH cand AS ( \
                SELECT target, COUNT(DISTINCT source) AS deg \
                FROM graph_edges \
                WHERE source LIKE 'session:%' \
                  AND (target LIKE 'file:%' OR target LIKE 'issue:%' OR target LIKE 'tech:%') \
                GROUP BY target \
                HAVING deg >= 2 AND deg <= ?1 \
            ) \
            SELECT e1.source AS a, e2.source AS b, cand.target AS ent, cand.deg AS deg \
            FROM graph_edges e1 \
            JOIN graph_edges e2 ON e1.target = e2.target AND e1.source < e2.source \
            JOIN cand ON cand.target = e1.target \
            WHERE e1.source LIKE 'session:%' AND e2.source LIKE 'session:%'";
        let mut stmt = self.conn().prepare(sql)?;
        let rows = stmt.query_map([deg_cap as i64], |r| {
            Ok((
                r.get::<_, String>(0)?, // a  (session:...)
                r.get::<_, String>(1)?, // b  (session:...)
                r.get::<_, String>(2)?, // ent (공유 엔티티 node_id)
                r.get::<_, i64>(3)?,    // deg (엔티티의 전체 세션 degree)
            ))
        })?;

        // (a,b) → (누적 score, [(공유 엔티티, weight)])
        let mut acc: HashMap<(String, String), PairAccum> = HashMap::new();
        for (a, b, ent, deg) in rows.filter_map(|r| r.ok()) {
            if deg < 2 {
                continue; // ln(1)=0 방어 (HAVING 이 걸러주지만 이중 안전)
            }
            let w = (1.0 / (deg as f64).ln()) as f32;
            let entry = acc.entry((a, b)).or_insert((0.0, Vec::new()));
            entry.0 += w;
            entry.1.push((ent, w));
        }

        // 세션 project 맵 — cross-project 필터 + 라벨 (sessions 테이블 통째, 수천 rows 라 가벼움).
        let proj_map: HashMap<String, Option<String>> = {
            let mut stmt = self.conn().prepare("SELECT id, project FROM sessions")?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let mut pairs: Vec<SurprisingPair> = acc
            .into_iter()
            .filter_map(|((a, b), (score, mut shared))| {
                let sa = a.strip_prefix("session:").unwrap_or(&a);
                let sb = b.strip_prefix("session:").unwrap_or(&b);
                let pa = proj_map.get(sa).cloned().flatten();
                let pb = proj_map.get(sb).cloned().flatten();
                // cross-project 만: 둘 다 project 가 있고 서로 다를 때 (같은 프로젝트/미상은 제외).
                // 대소문자 표기 변형(tunaFlow/tunaflow)은 동일 프로젝트로 간주 → 제외.
                match (&pa, &pb) {
                    (Some(x), Some(y)) if !x.eq_ignore_ascii_case(y) => {}
                    _ => return None,
                }
                // 공유 엔티티를 기여도(weight) 내림차순 → 근거 상위부터
                shared.sort_by(|x, y| {
                    y.1.partial_cmp(&x.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(x.0.cmp(&y.0))
                });
                Some(SurprisingPair {
                    session_a: sa.to_string(),
                    session_b: sb.to_string(),
                    project_a: pa,
                    project_b: pb,
                    score,
                    shared: shared.into_iter().map(|(e, _)| e).collect(),
                })
            })
            .collect();

        // score 내림차순 → 동점은 (a,b) 사전순 (결정성)
        pairs.sort_by(|x, y| {
            y.score
                .partial_cmp(&x.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(x.session_a.cmp(&y.session_a))
                .then(x.session_b.cmp(&y.session_b))
        });
        pairs.truncate(top_n);
        Ok(pairs)
    }

    /// 고립 세션 / 싱글턴 아티팩트 / 세션 degree 분포 (`graph_insights` 내부).
    fn knowledge_gaps(&self) -> Result<KnowledgeGaps> {
        const EXAMPLE_LIMIT: i64 = 10;

        // 1. 고립 세션: type='session' 노드 중 source 엣지가 없는 것.
        let isolated_session_count = self.conn().query_row(
            "SELECT COUNT(*) FROM graph_nodes n WHERE n.type='session' \
             AND NOT EXISTS (SELECT 1 FROM graph_edges e WHERE e.source = n.id)",
            [],
            |r| r.get::<_, i64>(0),
        )? as usize;

        let isolated_session_examples: Vec<String> = {
            let mut stmt = self.conn().prepare(
                "SELECT n.id FROM graph_nodes n WHERE n.type='session' \
                 AND NOT EXISTS (SELECT 1 FROM graph_edges e WHERE e.source = n.id) \
                 ORDER BY n.id LIMIT ?1",
            )?;
            let rows = stmt.query_map([EXAMPLE_LIMIT], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok())
                .map(|id| id.strip_prefix("session:").unwrap_or(&id).to_string())
                .collect()
        };

        // 2. 싱글턴 file/issue: deg=1 (한 세션만 참조).
        let count_singleton = |prefix: &str| -> Result<usize> {
            let sql = format!(
                "SELECT COUNT(*) FROM ( \
                   SELECT target FROM graph_edges \
                   WHERE source LIKE 'session:%' AND target LIKE '{}:%' \
                   GROUP BY target HAVING COUNT(DISTINCT source) = 1 \
                 )",
                prefix
            );
            let c: i64 = self.conn().query_row(&sql, [], |r| r.get(0))?;
            Ok(c as usize)
        };
        let singleton_file_count = count_singleton("file")?;
        let singleton_issue_count = count_singleton("issue")?;

        // 예시는 issue 우선(수가 적고 의미 큼) 후 file.
        let singleton_examples: Vec<String> = {
            let mut stmt = self.conn().prepare(
                "SELECT target FROM graph_edges \
                 WHERE source LIKE 'session:%' AND (target LIKE 'issue:%' OR target LIKE 'file:%') \
                 GROUP BY target HAVING COUNT(DISTINCT source) = 1 \
                 ORDER BY CASE WHEN target LIKE 'issue:%' THEN 0 ELSE 1 END, target LIMIT ?1",
            )?;
            let rows = stmt.query_map([EXAMPLE_LIMIT], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        // 3. 세션 degree 분포 (엣지≥1 세션의 연결 엔티티 수).
        let mut degrees: Vec<usize> = {
            let mut stmt = self.conn().prepare(
                "SELECT COUNT(*) FROM graph_edges WHERE source LIKE 'session:%' GROUP BY source",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
            rows.filter_map(|r| r.ok()).map(|d| d as usize).collect()
        };
        let (session_degree_min, session_degree_median, session_degree_max) = if degrees.is_empty()
        {
            (0, 0, 0)
        } else {
            degrees.sort_unstable();
            (
                degrees[0],
                degrees[degrees.len() / 2],
                degrees[degrees.len() - 1],
            )
        };

        // 4. 그래프 편입률 — sessions 테이블 대비 graph 미편입 세션 (최대 gap).
        let sessions_total = self
            .conn()
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))?
            as usize;
        let sessions_in_graph = self.conn().query_row(
            "SELECT COUNT(*) FROM graph_nodes WHERE type='session'",
            [],
            |r| r.get::<_, i64>(0),
        )? as usize;
        let sessions_missing_from_graph = sessions_total.saturating_sub(sessions_in_graph);

        Ok(KnowledgeGaps {
            isolated_session_count,
            isolated_session_examples,
            singleton_file_count,
            singleton_issue_count,
            singleton_examples,
            sessions_total,
            sessions_in_graph,
            sessions_missing_from_graph,
            session_degree_min,
            session_degree_median,
            session_degree_max,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Database;

    #[test]
    fn test_graph_insights_surprising_and_gaps() {
        let db = Database::open_memory().unwrap();

        // 세션 노드 4개 — iso 는 엣지 없음 → 고립
        for s in ["session:s1", "session:s2", "session:s3", "session:iso"] {
            db.upsert_graph_node(s, "session", s, None).unwrap();
        }
        // sessions 테이블 행 — cross-project 필터용 project 부여 (s1,s3=projA / s2=projB)
        insert_session_row(&db, "s1", "projA");
        insert_session_row(&db, "s2", "projB");
        insert_session_row(&db, "s3", "projA");
        insert_session_row(&db, "iso", "projA");
        // 엔티티 노드 (graph_edges FK 제약 충족)
        db.upsert_graph_node("file:shared", "file", "shared", None)
            .unwrap();
        db.upsert_graph_node("tech:common", "tech", "common", None)
            .unwrap();
        db.upsert_graph_node("issue:only1", "issue", "only1", None)
            .unwrap();
        db.upsert_graph_node("topic:x", "topic", "x", None).unwrap();

        // file:shared — s1,s2 (deg 2) → 강한 surprising 근거
        db.upsert_graph_edge(
            "session:s1",
            "file:shared",
            "modifies_file",
            "EXTRACTED",
            1.0,
        )
        .unwrap();
        db.upsert_graph_edge(
            "session:s2",
            "file:shared",
            "modifies_file",
            "EXTRACTED",
            1.0,
        )
        .unwrap();
        // tech:common — s1,s2,s3 (deg 3)
        for s in ["session:s1", "session:s2", "session:s3"] {
            db.upsert_graph_edge(s, "tech:common", "introduces_tech", "EXTRACTED", 1.0)
                .unwrap();
        }
        // issue:only1 — s1 만 (deg 1) → 싱글턴, surprising 후보 아님
        db.upsert_graph_edge("session:s1", "issue:only1", "fixes_bug", "EXTRACTED", 1.0)
            .unwrap();
        // topic:x — s1,s2 (deg 2) 이지만 topic 은 surprising 후보에서 제외돼야 함
        db.upsert_graph_edge("session:s1", "topic:x", "discusses_topic", "EXTRACTED", 1.0)
            .unwrap();
        db.upsert_graph_edge("session:s2", "topic:x", "discusses_topic", "EXTRACTED", 1.0)
            .unwrap();

        let ins = db.graph_insights(10, 25).unwrap();

        // ── surprising ── (s1,s2) 가 file:shared + tech:common 공유로 최상위
        assert!(!ins.surprising.is_empty());
        let top = &ins.surprising[0];
        assert_eq!(top.session_a, "s1");
        assert_eq!(top.session_b, "s2");
        // cross-project 라벨 (projA ↔ projB)
        assert_eq!(top.project_a.as_deref(), Some("projA"));
        assert_eq!(top.project_b.as_deref(), Some("projB"));
        // same-project 쌍 (s1,s3 = 둘 다 projA) 은 제외됨
        assert!(!ins
            .surprising
            .iter()
            .any(|p| p.session_a == "s1" && p.session_b == "s3"));
        assert!(top.shared.contains(&"file:shared".to_string()));
        assert!(top.shared.contains(&"tech:common".to_string()));
        // topic 은 후보에서 제외 → 근거에 없어야 함
        assert!(!top.shared.iter().any(|s| s.starts_with("topic:")));
        // score = 1/ln2 + 1/ln3 (내부 f64→f32 캐스트와 동일 방식으로 기대값 계산)
        let expect = (1.0f64 / 2f64.ln()) as f32 + (1.0f64 / 3f64.ln()) as f32;
        assert!((top.score - expect).abs() < 1e-3);
        // (s1,s2) 가 전체 최고 score
        for p in &ins.surprising[1..] {
            assert!(p.score <= top.score);
        }

        // ── gaps ──
        assert_eq!(ins.gaps.isolated_session_count, 1);
        assert_eq!(ins.gaps.isolated_session_examples, vec!["iso".to_string()]);
        assert_eq!(ins.gaps.singleton_issue_count, 1);
        assert!(ins
            .gaps
            .singleton_examples
            .contains(&"issue:only1".to_string()));
        // file:shared 는 deg 2 라 싱글턴 아님
        assert_eq!(ins.gaps.singleton_file_count, 0);
        // degree 분포: s1=4, s2=3, s3=1 (iso 제외) → min1/median3/max4
        assert_eq!(ins.gaps.session_degree_min, 1);
        assert_eq!(ins.gaps.session_degree_max, 4);
        // 그래프 편입률: 4세션 모두 노드 등록 → missing 0
        assert_eq!(ins.gaps.sessions_total, 4);
        assert_eq!(ins.gaps.sessions_in_graph, 4);
        assert_eq!(ins.gaps.sessions_missing_from_graph, 0);
    }

    /// sessions 테이블에 최소 세션 행 삽입 (cross-project 필터 테스트용).
    fn insert_session_row(db: &Database, id: &str, project: &str) {
        use crate::ingest::{AgentKind, Session, TokenUsage};
        use crate::store::SessionRepo;
        use chrono::Utc;
        let s = Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: Some(project.to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: Utc::now(),
            end_time: None,
            turns: Vec::new(),
            total_tokens: TokenUsage::default(),
            session_type: "interactive".to_string(),
            archived: false,
            archived_at: None,
        };
        db.insert_session(&s).unwrap();
    }

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

    #[test]
    fn test_adamic_adar_scores_rare_entity_higher() {
        let db = Database::open_memory().unwrap();
        for s in ["s1", "s2", "s3", "s4", "s5"] {
            db.upsert_graph_node(&format!("session:{s}"), "session", s, None)
                .unwrap();
        }
        db.upsert_graph_node("file:rare", "file", "rare.rs", None)
            .unwrap();
        db.upsert_graph_node("topic:common", "topic", "common", None)
            .unwrap();
        db.upsert_graph_node("agent:claude-code", "agent", "claude-code", None)
            .unwrap();
        // 희소 엔티티 file:rare — s1,s2 만 (deg=2)
        db.upsert_graph_edge("session:s1", "file:rare", "modifies_file", "RULE", 0.9)
            .unwrap();
        db.upsert_graph_edge("session:s2", "file:rare", "modifies_file", "RULE", 0.9)
            .unwrap();
        // 흔한 엔티티 topic:common — s1,s3,s4,s5 (deg=4)
        for s in ["s1", "s3", "s4", "s5"] {
            db.upsert_graph_edge(
                &format!("session:{s}"),
                "topic:common",
                "discusses_topic",
                "LLM",
                0.5,
            )
            .unwrap();
        }
        // agent 허브 — s1,s2,s3 (AA 에서 제외돼야 함)
        for s in ["s1", "s2", "s3"] {
            db.upsert_graph_edge(
                &format!("session:{s}"),
                "agent:claude-code",
                "by_agent",
                "RULE",
                1.0,
            )
            .unwrap();
        }

        let seed: std::collections::HashSet<String> =
            ["session:s1".to_string()].into_iter().collect();
        let cands: Vec<String> = ["session:s2", "session:s3"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let scores = db.adamic_adar_scores(&seed, &cands).unwrap();

        let s2 = scores.get("session:s2").copied().unwrap_or(0.0);
        let s3 = scores.get("session:s3").copied().unwrap_or(0.0);
        // 희소 file 공유(s2) > 흔한 topic 공유(s3)
        assert!(
            s2 > s3,
            "rare-entity share must score higher: s2={s2} s3={s3}"
        );
        // agent 허브는 기여 X → s2 는 file:rare 하나뿐 = 1/ln(2)
        assert!(
            (s2 - (1.0 / 2.0_f64.ln()) as f32).abs() < 1e-4,
            "s2 should equal 1/ln(2) (agent hub excluded): {s2}"
        );
    }
}
