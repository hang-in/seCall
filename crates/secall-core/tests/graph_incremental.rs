//! P33 Task 07 — graph 증분 추출 통합 테스트.
//!
//! `extract_for_sessions`가 DB에 등록된 vault_path를 읽어 세션 markdown을
//! 파싱하고 `graph_nodes` / `graph_edges`에 INSERT OR IGNORE로 누적되는지 검증한다.
//! cross-session 엣지(same_project, same_day)는 본 함수가 생성하지 않으므로
//! 검증 대상에서 제외한다.

use std::path::Path;

use secall_core::graph::extract::extract_for_sessions;
use secall_core::store::Database;

/// vault/raw/sessions/{date}/{id}.md를 작성하고 DB에 vault_path를 등록한다.
#[allow(clippy::too_many_arguments)]
fn write_session_md_and_register(
    db: &Database,
    vault: &Path,
    session_id: &str,
    date: &str,
    project: Option<&str>,
    agent: &str,
    tools: &[&str],
    body: &str,
) {
    let date_dir = vault.join("raw").join("sessions").join(date);
    std::fs::create_dir_all(&date_dir).unwrap();
    let rel = format!("raw/sessions/{date}/{session_id}.md");
    let abs = vault.join(&rel);

    let project_line = match project {
        Some(p) => format!("project: {p}\n"),
        None => String::new(),
    };
    let tools_line = if tools.is_empty() {
        String::new()
    } else {
        let list = tools
            .iter()
            .map(|t| format!("  - {t}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("tools_used:\n{list}\n")
    };

    let content = format!(
        "---\nsession_id: {session_id}\nagent: {agent}\n{project_line}date: {date}\nstart_time: {date}T00:00:00Z\nturns: 5\nsummary: test summary\n{tools_line}---\n\n{body}\n",
    );
    std::fs::write(&abs, content).unwrap();

    // DB sessions 테이블에 빈 row + vault_path만 셋업
    db.conn()
        .execute(
            "INSERT OR REPLACE INTO sessions (id, agent, model, project, cwd, host, start_time, end_time, turn_count, vault_path, session_type, ingested_at) VALUES (?1, ?2, NULL, ?3, NULL, NULL, ?4, NULL, 5, ?5, 'interactive', ?4)",
            rusqlite::params![
                session_id,
                agent,
                project,
                format!("{date}T00:00:00Z"),
                rel,
            ],
        )
        .unwrap();
}

#[test]
fn extract_for_sessions_adds_nodes_and_edges_for_new_sessions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault = tmp.path();
    let db = Database::open_memory().unwrap();

    write_session_md_and_register(
        &db,
        vault,
        "ses_increment_1",
        "2026-05-02",
        Some("tunaflow"),
        "claude-code",
        &["Edit", "Read"],
        "Body text without issue refs.",
    );

    let report = extract_for_sessions(&db, vault, &["ses_increment_1".to_string()]).unwrap();
    assert_eq!(report.sessions_processed, 1);
    // session, project, agent, tool(Edit), tool(Read) = 5 노드
    assert_eq!(report.nodes_added, 5);
    // belongs_to, by_agent, uses_tool×2 = 4 엣지
    assert_eq!(report.edges_added, 4);

    // graph_nodes에 session 노드 존재
    let session_node: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM graph_nodes WHERE id = 'session:ses_increment_1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(session_node, 1);
}

#[test]
fn extract_for_sessions_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault = tmp.path();
    let db = Database::open_memory().unwrap();

    write_session_md_and_register(
        &db,
        vault,
        "ses_idem",
        "2026-05-02",
        Some("proj"),
        "claude-code",
        &["Edit"],
        "Body.",
    );

    let r1 = extract_for_sessions(&db, vault, &["ses_idem".to_string()]).unwrap();
    assert!(r1.edges_added > 0);

    // 두 번째 호출: 모든 엣지가 INSERT OR IGNORE로 무시되어야 함
    let r2 = extract_for_sessions(&db, vault, &["ses_idem".to_string()]).unwrap();
    assert_eq!(
        r2.edges_added, 0,
        "두 번째 호출에서는 중복 엣지가 모두 무시되어야 함"
    );
    assert_eq!(r2.sessions_processed, 1);
}

#[test]
fn extract_for_sessions_skips_when_vault_path_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault = tmp.path();
    let db = Database::open_memory().unwrap();

    // vault_path를 등록하지 않음 — sessions row도 없음
    let report = extract_for_sessions(&db, vault, &["nonexistent".to_string()]).unwrap();
    assert_eq!(report.sessions_processed, 0);
    assert_eq!(report.nodes_added, 0);
    assert_eq!(report.edges_added, 0);
}

#[test]
fn extract_for_sessions_extracts_semantic_edges() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vault = tmp.path();
    let db = Database::open_memory().unwrap();

    // body에 fixes #42 가 있고, tools_used에 Edit이 있어야 modifies_file 추출됨
    write_session_md_and_register(
        &db,
        vault,
        "ses_sem",
        "2026-05-02",
        Some("proj"),
        "claude-code",
        &["Edit"],
        "fixes #42\n\n> [!tool]- Edit `src/main.rs`\n",
    );

    let report = extract_for_sessions(&db, vault, &["ses_sem".to_string()]).unwrap();
    assert_eq!(report.sessions_processed, 1);

    // fixes_bug 엣지 존재 확인
    let fixes_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE source = 'session:ses_sem' AND relation = 'fixes_bug'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fixes_count, 1, "fixes_bug 엣지가 1개 추가되어야 함");

    // modifies_file 엣지 존재 확인
    let modifies_count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE source = 'session:ses_sem' AND relation = 'modifies_file'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(modifies_count, 1, "modifies_file 엣지가 1개 추가되어야 함");

    // issue:42, file:src/main.rs 노드도 함께 생성되어야 함
    let issue_node: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM graph_nodes WHERE id = 'issue:42'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(issue_node, 1);
    let file_node: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM graph_nodes WHERE id = 'file:src/main.rs'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(file_node, 1);
}
