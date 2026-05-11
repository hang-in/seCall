//! P45 Task 06 — 세션 archive/restore round-trip + filter 회귀 통합 테스트.
//!
//! 시나리오:
//!   1. archive_round_trip_updates_db_and_vault_and_excludes_from_list
//!   2. restore_round_trip_clears_db_and_vault
//!   3. cross_host_archive_via_re_ingest_syncs_db
//!   4. archive_excludes_from_bm25_search

use secall_core::ingest::markdown::{extract_body_text, parse_session_frontmatter};
use secall_core::ingest::{AgentKind, Role, Session, TokenUsage, Turn};
use secall_core::search::bm25::{Bm25Indexer, SearchFilters};
use secall_core::search::tokenizer::LinderaKoTokenizer;
use secall_core::store::session_repo::SessionListFilter;
use secall_core::store::Database;
use secall_core::vault::Vault;
use tempfile::TempDir;

// ─── Harness ──────────────────────────────────────────────────────────────────

struct Harness {
    _dir: TempDir,
    db: Database,
    vault: Vault,
}

fn setup_harness() -> Harness {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("secall.sqlite");
    let db = Database::open(&db_path).expect("open db");
    let vault = Vault::new(dir.path().to_path_buf());
    vault.init().expect("init vault");
    Harness {
        _dir: dir,
        db,
        vault,
    }
}

fn make_session(id: &str, body_word: &str) -> Session {
    Session {
        id: id.to_string(),
        agent: AgentKind::ClaudeCode,
        model: Some("test".to_string()),
        project: Some("test-proj".to_string()),
        cwd: None,
        git_branch: None,
        host: None,
        start_time: chrono::DateTime::from_timestamp(1_747_000_000, 0).unwrap(),
        end_time: None,
        session_type: "interactive".to_string(),
        archived: false,
        archived_at: None,
        turns: vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: body_word.to_string(),
            actions: vec![],
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }],
        total_tokens: TokenUsage::default(),
    }
}

/// vault.write_session + db.insert_session_from_vault.
/// Returns vault-relative path string.
fn ingest_session(h: &Harness, id: &str, body_word: &str) -> String {
    let session = make_session(id, body_word);
    let rel_path = h
        .vault
        .write_session(&session, chrono_tz::UTC)
        .expect("write_session");
    let rel_str = rel_path.to_string_lossy().to_string();
    let abs = h.vault.path().join(&rel_path);
    let content = std::fs::read_to_string(&abs).unwrap();
    let fm = parse_session_frontmatter(&content).expect("parse fm");
    let body = extract_body_text(&content);
    h.db.insert_session_from_vault(&fm, &body, &rel_str)
        .expect("insert_session_from_vault");
    rel_str
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn archive_round_trip_updates_db_and_vault_and_excludes_from_list() {
    let h = setup_harness();
    let vault_rel = ingest_session(&h, "sess-rt-a", "hello world content");

    h.db.archive_session("sess-rt-a", &h.vault, chrono_tz::UTC)
        .expect("archive_session");

    // DB: is_archived = 1
    let is_archived: i64 = h
        .db
        .conn()
        .query_row(
            "SELECT is_archived FROM sessions WHERE id = 'sess-rt-a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(is_archived, 1);

    // Vault frontmatter: archived: true + archived_at 존재
    let abs = h.vault.path().join(&vault_rel);
    let content = std::fs::read_to_string(&abs).unwrap();
    assert!(
        content.contains("\narchived: true\n"),
        "vault should contain 'archived: true'"
    );
    assert!(
        content.contains("archived_at:"),
        "vault should contain 'archived_at:'"
    );

    // list_sessions_filtered 기본 (include_archived=false) → 제외
    let filter = SessionListFilter {
        page: 1,
        page_size: 10,
        ..Default::default()
    };
    let page = h.db.list_sessions_filtered(&filter).unwrap();
    assert!(
        page.items.iter().all(|it| it.id != "sess-rt-a"),
        "archived session must be excluded from default list"
    );

    // include_archived=true → 포함
    let filter_inc = SessionListFilter {
        page: 1,
        page_size: 10,
        include_archived: true,
        ..Default::default()
    };
    let page_inc = h.db.list_sessions_filtered(&filter_inc).unwrap();
    assert!(
        page_inc.items.iter().any(|it| it.id == "sess-rt-a"),
        "archived session must appear with include_archived=true"
    );
}

#[test]
fn restore_round_trip_clears_db_and_vault() {
    let h = setup_harness();
    let vault_rel = ingest_session(&h, "sess-rt-r", "restore test content");

    h.db.archive_session("sess-rt-r", &h.vault, chrono_tz::UTC)
        .unwrap();
    h.db.restore_session("sess-rt-r", &h.vault, chrono_tz::UTC)
        .expect("restore_session");

    // DB: is_archived = 0
    let is_archived: i64 = h
        .db
        .conn()
        .query_row(
            "SELECT is_archived FROM sessions WHERE id = 'sess-rt-r'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(is_archived, 0);

    // Vault frontmatter: archived: 라인 없어야 함
    let abs = h.vault.path().join(&vault_rel);
    let content = std::fs::read_to_string(&abs).unwrap();
    assert!(
        !content.contains("archived:"),
        "vault should not contain 'archived:' after restore"
    );

    // default list 에 다시 포함
    let filter = SessionListFilter {
        page: 1,
        page_size: 10,
        ..Default::default()
    };
    let page = h.db.list_sessions_filtered(&filter).unwrap();
    assert!(
        page.items.iter().any(|it| it.id == "sess-rt-r"),
        "restored session must appear in default list"
    );
}

#[test]
fn cross_host_archive_via_re_ingest_syncs_db() {
    let h = setup_harness();
    let vault_rel = ingest_session(&h, "sess-xh", "cross host test");

    // 다른 머신이 git push 한 frontmatter 변경 시뮬레이션:
    // 직접 vault 파일에 archived: true / archived_at: 삽입
    let abs = h.vault.path().join(&vault_rel);
    let content = std::fs::read_to_string(&abs).unwrap();
    let modified = content.replace(
        "session_id: sess-xh",
        "session_id: sess-xh\narchived: true\narchived_at: \"2026-05-12T15:00:00+00:00\"",
    );
    std::fs::write(&abs, &modified).unwrap();

    // re-ingest (insert_session_from_vault → UPDATE archive sync)
    let re_content = std::fs::read_to_string(&abs).unwrap();
    let fm = parse_session_frontmatter(&re_content).expect("parse re-ingested fm");
    let body = extract_body_text(&re_content);
    h.db.insert_session_from_vault(&fm, &body, &vault_rel)
        .expect("re-ingest");

    let is_archived: i64 = h
        .db
        .conn()
        .query_row(
            "SELECT is_archived FROM sessions WHERE id = 'sess-xh'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(is_archived, 1, "re-ingest must sync archived=true to DB");
}

#[test]
fn archive_excludes_from_bm25_search() {
    let h = setup_harness();
    ingest_session(&h, "sess-bm25-arc", "unique-archive-token-zyx content here");

    // archive
    h.db.archive_session("sess-bm25-arc", &h.vault, chrono_tz::UTC)
        .unwrap();

    // BM25 인덱싱 (ingest 때 insert_session_from_vault 가 FTS 에도 넣지만
    // DB 직접 archive 후 BM25Indexer.search 를 통해 filter 검증)
    let tok = LinderaKoTokenizer::new().unwrap();
    let indexer = Bm25Indexer::new(Box::new(tok));

    // include_archived=false (기본) → archived 세션 제외
    let hits = indexer
        .search(
            &h.db,
            "unique-archive-token-zyx",
            10,
            &SearchFilters::default(),
        )
        .unwrap();
    assert!(
        hits.iter().all(|r| r.session_id != "sess-bm25-arc"),
        "archived session must be excluded from BM25 search by default"
    );

    // include_archived=true → 포함
    let hits_inc = indexer
        .search(
            &h.db,
            "unique-archive-token-zyx",
            10,
            &SearchFilters {
                include_archived: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        hits_inc.iter().any(|r| r.session_id == "sess-bm25-arc"),
        "archived session must appear in BM25 search with include_archived=true"
    );
}
