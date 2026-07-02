use std::collections::HashSet;
use std::path::Path;

use crate::error::Result;
use crate::ingest::markdown::parse_session_frontmatter;
use crate::store::Database;

use super::extract::{extract_from_frontmatter, extract_semantic_edges, extract_session_relations};
use crate::ingest::markdown::extract_body_text;

#[derive(Debug, Default)]
pub struct BuildResult {
    pub nodes_created: usize,
    pub edges_created: usize,
    pub sessions_processed: usize,
    pub sessions_skipped: usize,
    /// 파일 읽기/파싱 실패로 건너뛴 세션 수
    pub sessions_failed: usize,
}

/// 경로 세그먼트가 YYYY-MM-DD 형식의 날짜 디렉토리명인지 검증한다.
/// 예: "2026-04-10" → true, "session001" → false, "2026-99-99" → true (범위 불검사)
fn is_date_dir(name: &str) -> bool {
    name.len() == 10
        && name.as_bytes()[4] == b'-'
        && name.as_bytes()[7] == b'-'
        && name[..4].chars().all(|c| c.is_ascii_digit())
        && name[5..7].chars().all(|c| c.is_ascii_digit())
        && name[8..10].chars().all(|c| c.is_ascii_digit())
}

/// vault 전체에서 그래프를 빌드하거나 증분 갱신한다.
///
/// - `since`: YYYY-MM-DD 이상인 세션만 노드/엣지 full upsert 대상 (부모 디렉토리명 기준).
///   관계 계산(same_project/same_day)은 since 무관하게 **전체 vault** 대상으로 수행.
///   since 이전이고 DB에 없는 세션은 FK 제약 충족을 위해 session 노드만 최소 삽입.
/// - `force`: true이면 기존 그래프를 삭제 후 전체 재빌드
pub fn build_graph(
    db: &Database,
    vault_path: &Path,
    since: Option<&str>,
    force: bool,
) -> Result<BuildResult> {
    let sessions_dir = crate::vault::sessions_subdir(vault_path);
    if !sessions_dir.exists() {
        return Ok(BuildResult::default());
    }

    if force {
        db.clear_graph()?;
    }

    // 이미 처리된 세션 ID 세트
    let already_graphed: HashSet<String> = db.list_graphed_session_ids()?.into_iter().collect();

    // vault/raw/sessions/ 순회
    let md_files: Vec<_> = walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
        .collect();

    // all_frontmatters: 관계 계산 대상 — 전체 vault 세션 (since 무관).
    // is_new: full upsert 대상 여부 (all_frontmatters와 1:1 대응).
    // needs_minimal_node: DB에 없고 is_new도 아닌 세션 — FK 충족을 위해 session 노드만 삽입.
    // bodies: 시맨틱 엣지 추출용 본문 (is_new=true인 세션만 유효, 나머지는 빈 문자열)
    let mut all_frontmatters = Vec::new();
    let mut is_new: Vec<bool> = Vec::new();
    let mut needs_minimal_node: Vec<bool> = Vec::new();
    let mut bodies: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for entry in &md_files {
        let path = entry.path();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to read session file");
                failed += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
                failed += 1;
                continue;
            }
        };

        // since 필터: 부모 디렉토리명이 YYYY-MM-DD 형식인 경우만 날짜 비교.
        // 파일명 등 날짜 디렉토리 형식이 아닌 세그먼트는 무시 (항상 포함).
        let in_since_range = if let Some(since_date) = since {
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            !is_date_dir(parent_name) || parent_name >= since_date
        } else {
            true
        };

        // is_new 판정: force이거나, (미처리 && since 범위 내)
        let already = already_graphed.contains(&fm.session_id);
        let session_is_new = force || (!already && in_since_range);

        if !session_is_new {
            skipped += 1;
        }

        // needs_minimal_node: DB에 없고 is_new도 아닌 세션.
        // → 관계 엣지의 FK 제약을 충족하기 위해 session 노드를 최소 형태로 삽입.
        // → since 이전 + fresh DB 케이스(계약: 관계 계산은 전체 vault 대상)에서 발생.
        // force=true이면 모든 세션이 is_new이므로 needs_minimal은 항상 false.
        let minimal = !session_is_new && !already;

        // 모든 세션을 관계 계산에 포함 (since 이전 세션도 참여)
        // 시맨틱 엣지 추출을 위해 신규 세션의 body를 보존, 나머지는 빈 문자열
        let body = if session_is_new {
            extract_body_text(&content)
        } else {
            String::new()
        };
        all_frontmatters.push(fm);
        is_new.push(session_is_new);
        needs_minimal_node.push(minimal);
        bodies.push(body);
    }

    // 새로 처리한 세션 수
    let sessions_processed = is_new.iter().filter(|&&b| b).count();
    let mut total_nodes = 0usize;
    let mut total_edges = 0usize;

    db.with_transaction(|| {
        // [핵심 수정 1] 관계 엣지를 전체 삭제 후 재계산.
        // 이유: 인접 관계(A→B→C)는 전체 순서에 의존하므로 부분 갱신 불가.
        // 중간 세션 B 추가 시 기존 A→C를 삭제하고 A→B, B→C로 교체해야 함.
        db.delete_relation_edges(&["same_project", "same_day"])?;

        // [핵심 수정 2] since 이전이고 DB에 없는 세션의 session 노드를 최소 삽입.
        // 관계 엣지(same_project/same_day)의 source/target이 반드시 graph_nodes에 존재해야 함.
        // full upsert는 아래 루프에서 수행하므로 여기서는 session 노드만 삽입.
        for (fm, &minimal) in all_frontmatters.iter().zip(needs_minimal_node.iter()) {
            if minimal {
                let session_node_id = format!("session:{}", fm.session_id);
                let label = fm.session_id[..fm.session_id.len().min(8)].to_string();
                db.upsert_graph_node(&session_node_id, "session", &label, None)?;
                // total_nodes에 미포함: sessions_processed 카운트에 영향 없음
            }
        }

        // 개별 노드/엣지: 신규 세션만 full upsert
        for ((fm, &new_session), body) in all_frontmatters
            .iter()
            .zip(is_new.iter())
            .zip(bodies.iter())
        {
            if !new_session {
                continue;
            }
            let result = extract_from_frontmatter(fm);
            for node in &result.nodes {
                db.upsert_graph_node(&node.id, &node.node_type, &node.label, node.meta.as_deref())?;
                total_nodes += 1;
            }
            for edge in &result.edges {
                db.upsert_graph_edge(
                    &edge.source,
                    &edge.target,
                    &edge.relation,
                    &edge.confidence,
                    edge.weight,
                )?;
                total_edges += 1;
            }

            // 시맨틱 엣지 (P1 rule-based): fixes_bug, modifies_file
            let semantic = extract_semantic_edges(fm, body);
            for edge in &semantic {
                // 타겟 노드 자동 생성 (issue:N, file:path)
                let (target_type, target_label) =
                    if let Some(num) = edge.target.strip_prefix("issue:") {
                        ("issue", num)
                    } else if let Some(path) = edge.target.strip_prefix("file:") {
                        ("file", path)
                    } else {
                        ("unknown", edge.target.as_str())
                    };
                db.upsert_graph_node(&edge.target, target_type, target_label, None)?;
                db.upsert_graph_edge(
                    &edge.source,
                    &edge.target,
                    &edge.relation,
                    &edge.confidence,
                    edge.weight,
                )?;
                total_edges += 1;
            }
        }

        // 세션 간 관계 엣지: 전체 vault 세션 대상으로 재계산 후 삽입
        let relation_edges = extract_session_relations(&all_frontmatters);
        for edge in &relation_edges {
            db.upsert_graph_edge(
                &edge.source,
                &edge.target,
                &edge.relation,
                &edge.confidence,
                edge.weight,
            )?;
            total_edges += 1;
        }

        Ok(())
    })?;

    Ok(BuildResult {
        nodes_created: total_nodes,
        edges_created: total_edges,
        sessions_processed,
        sessions_skipped: skipped,
        sessions_failed: failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Database;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_session_md(dir: &Path, session_id: &str, date: &str, project: &str) {
        let date_dir = crate::vault::sessions_subdir(dir).join(date);
        std::fs::create_dir_all(&date_dir).unwrap();
        let path = date_dir.join(format!("{}.md", session_id));
        let content = format!(
            "---\nsession_id: {}\nagent: claude-code\nproject: {}\ndate: {}\nstart_time: {}T00:00:00Z\n---\n\nBody text.",
            session_id, project, date, date
        );
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_build_graph_incremental() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        write_session_md(vault_path, "session001", "2026-04-10", "proj1");
        write_session_md(vault_path, "session002", "2026-04-10", "proj1");

        let db = Database::open_memory().unwrap();

        // 첫 빌드
        let r1 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r1.sessions_processed, 2);
        assert_eq!(r1.sessions_skipped, 0);

        // 두 번째 빌드 — 이미 처리된 세션은 스킵
        let r2 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r2.sessions_processed, 0);
        assert_eq!(r2.sessions_skipped, 2);
    }

    /// 증분 빌드에서 신규 세션과 기존 세션 간의 same_project 엣지가
    /// 올바르게 생성되는지 검증한다.
    #[test]
    fn test_incremental_build_creates_cross_session_relations() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        // 기존 세션 1개 — 첫 빌드에서 처리
        write_session_md(vault_path, "old001", "2026-04-09", "proj1");

        let db = Database::open_memory().unwrap();

        let r1 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r1.sessions_processed, 1);

        // 같은 project에 신규 세션 추가
        write_session_md(vault_path, "new001", "2026-04-10", "proj1");

        let r2 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r2.sessions_processed, 1); // 신규 1개만 처리
        assert_eq!(r2.sessions_skipped, 1); // 기존 1개는 스킵

        // 기존(old001) ↔ 신규(new001) 간 same_project 엣지 존재 확인
        let old_neighbors = db.get_neighbors("session:old001").unwrap();
        let new_neighbors = db.get_neighbors("session:new001").unwrap();

        let old_has_new = old_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:new001" && rel == "same_project");
        let new_has_old = new_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:old001" && rel == "same_project");

        assert!(
            old_has_new || new_has_old,
            "기존 세션과 신규 세션 사이에 same_project 엣지가 있어야 합니다.\n\
             old_neighbors: {:?}\nnew_neighbors: {:?}",
            old_neighbors,
            new_neighbors
        );
    }

    /// since 필터가 파일명(session001 등 10글자)을 날짜로 오인하지 않는지 검증.
    /// 부모 디렉토리명(YYYY-MM-DD)만 since 필터에 사용해야 한다.
    #[test]
    fn test_since_filter_only_matches_date_dirs() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        // 파일명이 10글자이고 lexicographic으로 since_date보다 큰 케이스
        // 구 코드에서는 "session001" >= "2026-04-10" → 's' > '2' → true (오탐)
        // 수정 후: 부모 디렉토리 "2026-04-09" < "2026-04-10" → is_new=false
        write_session_md(vault_path, "session001", "2026-04-09", "proj1");
        write_session_md(vault_path, "session002", "2026-04-10", "proj1");

        let db = Database::open_memory().unwrap();
        let r = build_graph(&db, vault_path, Some("2026-04-10"), false).unwrap();

        // session002만 since 범위 내 → full upsert
        assert_eq!(r.sessions_processed, 1, "since 범위 내 세션은 1개여야 함");
        assert_eq!(r.sessions_skipped, 1, "since 범위 밖 세션은 1개여야 함");
    }

    /// 중간 세션 추가 시 인접 엣지가 올바르게 교체되는지 검증.
    /// A→C 엣지가 A→B, B→C로 대체되어야 한다.
    #[test]
    fn test_incremental_replaces_adjacency_edges() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        // 1회차: A(04-09), C(04-11) — 같은 project
        write_session_md(vault_path, "sess_a", "2026-04-09", "proj1");
        write_session_md(vault_path, "sess_c", "2026-04-11", "proj1");

        let db = Database::open_memory().unwrap();
        let r1 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r1.sessions_processed, 2);

        // A→C 엣지가 존재해야 함
        let a_neighbors_1 = db.get_neighbors("session:sess_a").unwrap();
        let a_has_c = a_neighbors_1
            .iter()
            .any(|(id, rel, _)| id == "session:sess_c" && rel == "same_project");
        assert!(a_has_c, "1회차: A→C same_project 엣지가 있어야 함");

        // 2회차: 중간 세션 B(04-10) 추가
        write_session_md(vault_path, "sess_b", "2026-04-10", "proj1");
        let r2 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r2.sessions_processed, 1); // B만 신규

        // A→C 엣지가 삭제되고, A→B, B→C로 교체되어야 함
        let a_neighbors_2 = db.get_neighbors("session:sess_a").unwrap();
        let b_neighbors_2 = db.get_neighbors("session:sess_b").unwrap();

        let a_has_c_after = a_neighbors_2
            .iter()
            .any(|(id, rel, _)| id == "session:sess_c" && rel == "same_project");
        let a_has_b = a_neighbors_2
            .iter()
            .any(|(id, rel, _)| id == "session:sess_b" && rel == "same_project");
        let b_has_c = b_neighbors_2
            .iter()
            .any(|(id, rel, _)| id == "session:sess_c" && rel == "same_project");

        assert!(!a_has_c_after, "2회차: A→C 엣지는 삭제되어야 함");
        assert!(a_has_b, "2회차: A→B 엣지가 있어야 함");
        assert!(b_has_c, "2회차: B→C 엣지가 있어야 함");
    }

    /// since가 관계 계산 범위를 제한하지 않는지 검증 (이전 빌드 세션 포함).
    /// since 이전이어도 이전 빌드에서 이미 처리된 세션은 관계 계산에 참여해야 한다.
    #[test]
    fn test_since_does_not_limit_relation_scope() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        // 1회차: OLD(04-08, proj1) — 빌드 완료
        write_session_md(vault_path, "old_sess", "2026-04-08", "proj1");
        let db = Database::open_memory().unwrap();
        let r1 = build_graph(&db, vault_path, None, false).unwrap();
        assert_eq!(r1.sessions_processed, 1);

        // 2회차: NEW(04-10, proj1) — since="2026-04-10"으로 빌드
        write_session_md(vault_path, "new_sess", "2026-04-10", "proj1");
        let r2 = build_graph(&db, vault_path, Some("2026-04-10"), false).unwrap();
        assert_eq!(r2.sessions_processed, 1); // NEW만 full upsert
        assert_eq!(r2.sessions_skipped, 1); // OLD는 이미 처리됨

        // 관계 계산은 OLD+NEW 전체 대상이므로 same_project 엣지 존재해야 함
        let old_neighbors = db.get_neighbors("session:old_sess").unwrap();
        let new_neighbors = db.get_neighbors("session:new_sess").unwrap();

        let old_has_new = old_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:new_sess" && rel == "same_project");
        let new_has_old = new_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:old_sess" && rel == "same_project");

        assert!(
            old_has_new || new_has_old,
            "since 이전 세션(OLD)과 신규 세션(NEW) 간에 same_project 엣지가 있어야 함.\n\
             old_neighbors: {:?}\nnew_neighbors: {:?}",
            old_neighbors,
            new_neighbors
        );
    }

    /// fresh DB + --since 조합에서도 since 이전 세션이 관계 계산에 참여하는지 검증.
    /// 계약: since는 노드 full upsert에만 적용, 관계 계산은 전체 vault 대상.
    /// since 이전 + fresh DB 세션은 session 노드를 최소 삽입 후 관계 계산에 포함.
    #[test]
    fn test_since_includes_prior_sessions_in_relations_fresh_db() {
        let tmp = TempDir::new().unwrap();
        let vault_path = tmp.path();

        // fresh DB: old_sess(04-08)과 new_sess(04-10)이 함께 vault에 존재
        write_session_md(vault_path, "old_sess", "2026-04-08", "proj1");
        write_session_md(vault_path, "new_sess", "2026-04-10", "proj1");

        let db = Database::open_memory().unwrap();
        // 처음부터 since="2026-04-10"으로 빌드 (old_sess는 미처리 + since 범위 밖)
        let r = build_graph(&db, vault_path, Some("2026-04-10"), false).unwrap();

        assert_eq!(r.sessions_processed, 1, "new_sess만 full upsert 대상");
        assert_eq!(r.sessions_skipped, 1, "old_sess는 since 범위 밖");

        // old_sess는 최소 session 노드로 삽입되어 관계 계산에 참여해야 함
        // → old_sess ↔ new_sess 간 same_project 엣지가 존재해야 함
        let old_neighbors = db.get_neighbors("session:old_sess").unwrap();
        let new_neighbors = db.get_neighbors("session:new_sess").unwrap();

        let old_has_new = old_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:new_sess" && rel == "same_project");
        let new_has_old = new_neighbors
            .iter()
            .any(|(id, rel, _)| id == "session:old_sess" && rel == "same_project");

        assert!(
            old_has_new || new_has_old,
            "fresh DB + --since 에서도 since 이전 세션과 이후 세션 간 same_project 엣지가 있어야 함.\n\
             old_neighbors: {:?}\nnew_neighbors: {:?}",
            old_neighbors,
            new_neighbors
        );
    }
}
