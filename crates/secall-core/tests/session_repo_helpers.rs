//! P38 Task 03 — `session_repo` helper 회귀 통합 테스트.
//!
//! P32~P37 동안 추가된 session_repo helper 들을 한 자리에 모아 회귀한다.
//! 본 파일은 신규 helper 추가 시 가장 먼저 회귀를 추가하는 "단일 진입점" 역할이다.
//! 기존 inline 테스트 (`tests/rest_listing.rs` 등) 와 일부 중복은 의도적.
//!
//! 카테고리 (총 28 tests):
//!   1. 태그/즐겨찾기/노트 (P32+P34)        — 8 tests
//!   2. 필터링 (P32+P34)                     — 6 tests
//!   3. 통계 (P34)                           — 3 tests
//!   4. /api/tags (P35)                      — 3 tests
//!   5. 그래프 sync (P37)                    — 5 tests
//!   6. 메타 (P32)                           — 3 tests
//!
//! Task 00 의 `tests/common/mod.rs` 인프라는 의도적으로 사용하지 않는다.
//! Integration test crate 가 자동으로 분리 컴파일되므로 본 파일에 fixture 를 복제한다.

use chrono::TimeZone;
use secall_core::ingest::{Action, AgentKind, Role, Session, TokenUsage, Turn};
use secall_core::store::session_repo::{GraphRebuildFilter, SessionListFilter};
use secall_core::store::{Database, SessionRepo};

// ─── Fixtures (본 파일 자체 복제) ─────────────────────────────────────────────

fn make_db() -> Database {
    Database::open_memory().expect("open in-memory db")
}

/// `tests/rest_listing.rs::make_session` 와 동일 패턴.
/// project / day_offset 만 변하는 minimal Session.
fn make_session(id: &str, project: &str, day_offset: u32) -> Session {
    Session {
        id: id.to_string(),
        agent: AgentKind::ClaudeCode,
        model: Some("claude-sonnet-4-6".to_string()),
        project: Some(project.to_string()),
        cwd: None,
        git_branch: None,
        host: None,
        start_time: chrono::Utc
            .with_ymd_and_hms(2026, 5, 1 + day_offset, 0, 0, 0)
            .unwrap(),
        end_time: None,
        turns: vec![],
        total_tokens: TokenUsage::default(),
        session_type: "interactive".to_string(),
    }
}

/// 최소 정보로 세션 row 만 INSERT (insert_session 위임).
fn insert_minimal_session(db: &Database, id: &str, project: &str, day_offset: u32) {
    db.insert_session(&make_session(id, project, day_offset))
        .expect("insert minimal session");
}

fn make_turn(index: u32, role: Role, content: &str, tool_names: &[&str]) -> Turn {
    let actions = tool_names
        .iter()
        .map(|name| Action::ToolUse {
            name: (*name).to_string(),
            input_summary: String::new(),
            output_summary: String::new(),
            tool_use_id: None,
        })
        .collect();
    Turn {
        index,
        role,
        timestamp: None,
        content: content.to_string(),
        actions,
        tokens: None,
        thinking: None,
        is_sidechain: false,
    }
}

fn default_filter() -> SessionListFilter {
    SessionListFilter {
        page: 1,
        page_size: 30,
        ..Default::default()
    }
}

// ─── 1. 태그/즐겨찾기/노트 (P32 + P34) — 8 tests ──────────────────────────────

#[test]
fn tags_normalize_lowercase_and_hyphenate() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    let normalized = db
        .update_session_tags("s-1", &["Rust".into(), "hello world".into()])
        .unwrap();

    assert_eq!(normalized, vec!["hello-world", "rust"]);
}

#[test]
fn tags_dedup_preserves_single_entry() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    let normalized = db
        .update_session_tags("s-1", &["Rust".into(), "RUST".into(), "rust".into()])
        .unwrap();

    assert_eq!(normalized, vec!["rust"]);
}

#[test]
fn tags_empty_array_clears_all_tags() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);
    db.update_session_tags("s-1", &["alpha".into(), "beta".into()])
        .unwrap();

    let normalized = db.update_session_tags("s-1", &[]).unwrap();
    assert!(normalized.is_empty());

    let item = db.get_session_list_item("s-1").unwrap();
    assert!(item.tags.is_empty());
}

#[test]
fn tags_missing_session_returns_error() {
    let db = make_db();
    let res = db.update_session_tags("missing", &["x".into()]);
    assert!(res.is_err());
}

#[test]
fn favorite_toggles_true_then_false() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    db.update_session_favorite("s-1", true).unwrap();
    assert!(db.get_session_list_item("s-1").unwrap().is_favorite);

    db.update_session_favorite("s-1", false).unwrap();
    assert!(!db.get_session_list_item("s-1").unwrap().is_favorite);
}

#[test]
fn favorite_missing_session_returns_error() {
    let db = make_db();
    assert!(db.update_session_favorite("missing", true).is_err());
}

#[test]
fn notes_text_and_null_round_trip() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    db.update_session_notes("s-1", Some("free-form **markdown**"))
        .unwrap();
    assert_eq!(
        db.get_session_list_item("s-1").unwrap().notes.as_deref(),
        Some("free-form **markdown**")
    );

    db.update_session_notes("s-1", None).unwrap();
    assert!(db.get_session_list_item("s-1").unwrap().notes.is_none());
}

#[test]
fn notes_missing_session_returns_error() {
    let db = make_db();
    let res = db.update_session_notes("missing", Some("hello"));
    assert!(res.is_err());
}

// ─── 2. 필터링 (P32 + P34) — 6 tests ─────────────────────────────────────────

#[test]
fn filter_by_project_only() {
    let db = make_db();
    insert_minimal_session(&db, "s-a-0", "alpha", 0);
    insert_minimal_session(&db, "s-a-1", "alpha", 1);
    insert_minimal_session(&db, "s-b-0", "beta", 2);

    let mut f = default_filter();
    f.project = Some("alpha".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 2);
    assert!(page
        .items
        .iter()
        .all(|i| i.project.as_deref() == Some("alpha")));
}

#[test]
fn filter_by_agent_only() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);
    insert_minimal_session(&db, "s-2", "p", 1);

    let mut f = default_filter();
    f.agent = Some("claude-code".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 2);

    f.agent = Some("codex".to_string());
    assert_eq!(db.list_sessions_filtered(&f).unwrap().total, 0);
}

#[test]
fn filter_by_single_tag_matches_normalized_value() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);
    insert_minimal_session(&db, "s-2", "p", 1);
    db.update_session_tags("s-1", &["Rust".into()]).unwrap();

    let mut f = default_filter();
    f.tag = Some("rust".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "s-1");
}

#[test]
fn filter_by_multi_tag_requires_all() {
    let db = make_db();
    insert_minimal_session(&db, "s-both", "p", 0);
    insert_minimal_session(&db, "s-only-rust", "p", 1);
    insert_minimal_session(&db, "s-only-search", "p", 2);

    db.update_session_tags("s-both", &["rust".into(), "search".into()])
        .unwrap();
    db.update_session_tags("s-only-rust", &["rust".into()])
        .unwrap();
    db.update_session_tags("s-only-search", &["search".into()])
        .unwrap();

    let mut f = default_filter();
    f.tags = vec!["rust".into(), "search".into()];
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "s-both");
}

#[test]
fn filter_by_favorite_combined_with_project() {
    let db = make_db();
    insert_minimal_session(&db, "s-a-fav", "alpha", 0);
    insert_minimal_session(&db, "s-a-noFav", "alpha", 1);
    insert_minimal_session(&db, "s-b-fav", "beta", 2);
    db.update_session_favorite("s-a-fav", true).unwrap();
    db.update_session_favorite("s-b-fav", true).unwrap();

    let mut f = default_filter();
    f.project = Some("alpha".to_string());
    f.favorite = Some(true);
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "s-a-fav");
}

/// P38 rework — `SessionListFilter.date_from` (since) 필터 단독 회귀.
/// `make_session(day_offset)` 로 session 별 start_time 분리.
#[test]
fn filter_by_since_date_includes_only_recent() {
    let db = make_db();
    insert_minimal_session(&db, "s-day-0", "p", 0); // 2026-05-01
    insert_minimal_session(&db, "s-day-2", "p", 2); // 2026-05-03
    insert_minimal_session(&db, "s-day-4", "p", 4); // 2026-05-05

    // since = 2026-05-03 → s-day-2 + s-day-4 두 건만
    let mut f = default_filter();
    f.date_from = Some("2026-05-03".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 2, "since 필터 매칭 2건이어야 함, page={page:?}");
    let ids: Vec<&str> = page.items.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"s-day-2"));
    assert!(ids.contains(&"s-day-4"));
    assert!(!ids.contains(&"s-day-0"));

    // 미래 날짜 → 매칭 0
    f.date_from = Some("2099-01-01".to_string());
    assert_eq!(db.list_sessions_filtered(&f).unwrap().total, 0);

    // 매우 과거 → 모든 row
    f.date_from = Some("2000-01-01".to_string());
    assert_eq!(db.list_sessions_filtered(&f).unwrap().total, 3);
}

#[test]
fn filter_pagination_returns_correct_offset_and_size() {
    let db = make_db();
    for i in 0..5u32 {
        insert_minimal_session(&db, &format!("s-{i}"), "p", i);
    }

    let mut f = default_filter();
    f.page_size = 2;
    f.page = 1;
    let p1 = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(p1.total, 5);
    assert_eq!(p1.items.len(), 2);
    // ORDER BY start_time DESC — 가장 최근(s-4)이 첫
    assert_eq!(p1.items[0].id, "s-4");

    f.page = 3;
    let p3 = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(p3.items.len(), 1);
    assert_eq!(p3.items[0].id, "s-0");
}

// ─── 3. 통계 (P34) — 3 tests ─────────────────────────────────────────────────

#[test]
fn stats_for_session_with_no_turns_is_zero() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    let stats = db.get_session_stats("s-1").unwrap();
    assert_eq!(stats.user_turns, 0);
    assert_eq!(stats.assistant_turns, 0);
    assert_eq!(stats.system_turns, 0);
    assert!(stats.tool_counts.is_empty());
}

#[test]
fn stats_role_distribution_counts_each_role() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    db.insert_turn("s-1", &make_turn(0, Role::User, "hi", &[]))
        .unwrap();
    db.insert_turn("s-1", &make_turn(1, Role::Assistant, "hello", &[]))
        .unwrap();
    db.insert_turn("s-1", &make_turn(2, Role::User, "more", &[]))
        .unwrap();
    db.insert_turn("s-1", &make_turn(3, Role::System, "sys", &[]))
        .unwrap();

    let stats = db.get_session_stats("s-1").unwrap();
    assert_eq!(stats.user_turns, 2);
    assert_eq!(stats.assistant_turns, 1);
    assert_eq!(stats.system_turns, 1);
}

#[test]
fn stats_tool_counts_sorted_desc_then_alpha() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    db.insert_turn(
        "s-1",
        &make_turn(0, Role::Assistant, "t0", &["Edit", "Read"]),
    )
    .unwrap();
    db.insert_turn("s-1", &make_turn(1, Role::Assistant, "t1", &["Edit"]))
        .unwrap();
    db.insert_turn(
        "s-1",
        &make_turn(2, Role::Assistant, "t2", &["Read", "Bash"]),
    )
    .unwrap();

    let stats = db.get_session_stats("s-1").unwrap();
    assert_eq!(stats.tool_counts[0], ("Edit".to_string(), 2));
    assert_eq!(stats.tool_counts[1], ("Read".to_string(), 2));
    assert_eq!(stats.tool_counts[2], ("Bash".to_string(), 1));
}

// ─── 4. /api/tags (P35) — 3 tests ────────────────────────────────────────────

#[test]
fn list_all_tags_empty_db_returns_empty_vec() {
    let db = make_db();
    assert!(db.list_all_tags().unwrap().is_empty());
}

#[test]
fn list_all_tags_single_session_single_tag() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);
    db.update_session_tags("s-1", &["solo".into()]).unwrap();

    let tags = db.list_all_tags().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "solo");
    assert_eq!(tags[0].count, 1);
}

#[test]
fn list_all_tags_orders_by_count_desc_then_name_asc() {
    let db = make_db();
    insert_minimal_session(&db, "s1", "p", 0);
    insert_minimal_session(&db, "s2", "p", 1);
    insert_minimal_session(&db, "s3", "p", 2);

    db.update_session_tags("s1", &["rust".into(), "alpha".into()])
        .unwrap();
    db.update_session_tags("s2", &["rust".into(), "search".into()])
        .unwrap();
    db.update_session_tags("s3", &["rust".into()]).unwrap();

    let tags = db.list_all_tags().unwrap();
    assert_eq!(tags.len(), 3);
    // rust(3) > alpha(1)/search(1) — count tie 시 name ASC
    assert_eq!(tags[0].name, "rust");
    assert_eq!(tags[0].count, 3);
    assert_eq!(tags[1].name, "alpha");
    assert_eq!(tags[2].name, "search");
}

// ─── 5. 그래프 sync (P37) — 5 tests ──────────────────────────────────────────

#[test]
fn graph_rebuild_session_id_returns_single_match() {
    let db = make_db();
    insert_minimal_session(&db, "s-target", "p", 0);
    insert_minimal_session(&db, "s-other", "p", 1);

    let f = GraphRebuildFilter {
        session: Some("s-target".to_string()),
        ..Default::default()
    };
    let ids = db.list_sessions_for_graph_rebuild(f).unwrap();
    assert_eq!(ids, vec!["s-target".to_string()]);
}

#[test]
fn graph_rebuild_all_returns_every_session() {
    let db = make_db();
    insert_minimal_session(&db, "s-0", "p", 0);
    insert_minimal_session(&db, "s-1", "p", 1);
    insert_minimal_session(&db, "s-2", "p", 2);

    let f = GraphRebuildFilter {
        all: true,
        ..Default::default()
    };
    let mut ids = db.list_sessions_for_graph_rebuild(f).unwrap();
    ids.sort();
    assert_eq!(ids, vec!["s-0", "s-1", "s-2"]);
}

#[test]
fn graph_rebuild_retry_failed_returns_only_unprocessed() {
    let db = make_db();
    insert_minimal_session(&db, "s-done", "p", 0);
    insert_minimal_session(&db, "s-pending", "p", 1);
    db.update_semantic_extracted_at("s-done", 1_700_000_000)
        .unwrap();

    let f = GraphRebuildFilter {
        retry_failed: true,
        ..Default::default()
    };
    let ids = db.list_sessions_for_graph_rebuild(f).unwrap();
    assert_eq!(ids, vec!["s-pending".to_string()]);
}

#[test]
fn graph_rebuild_since_filters_by_start_time() {
    let db = make_db();
    insert_minimal_session(&db, "s-old", "p", 0); // 2026-05-01
    insert_minimal_session(&db, "s-new", "p", 5); // 2026-05-06

    let f = GraphRebuildFilter {
        since: Some("2026-05-03T00:00:00+00:00".to_string()),
        ..Default::default()
    };
    let ids = db.list_sessions_for_graph_rebuild(f).unwrap();
    assert_eq!(ids, vec!["s-new".to_string()]);
}

#[test]
fn graph_rebuild_default_filter_returns_empty() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "p", 0);

    let ids = db
        .list_sessions_for_graph_rebuild(GraphRebuildFilter::default())
        .unwrap();
    assert!(ids.is_empty());
}

#[test]
fn semantic_extracted_at_missing_session_is_no_op() {
    let db = make_db();
    // 미존재 세션에 대해 호출해도 에러 안 남 — 0 affected.
    let res = db.update_semantic_extracted_at("missing", 1_700_000_000);
    assert!(res.is_ok());
}

// ─── 6. 메타 (P32) — 3 tests ─────────────────────────────────────────────────

#[test]
fn get_session_list_item_returns_meta_fields() {
    let db = make_db();
    insert_minimal_session(&db, "s-meta", "alpha", 0);
    db.update_session_tags("s-meta", &["alpha".into(), "Beta".into()])
        .unwrap();
    db.update_session_favorite("s-meta", true).unwrap();

    let item = db.get_session_list_item("s-meta").unwrap();
    assert_eq!(item.id, "s-meta");
    assert_eq!(item.project.as_deref(), Some("alpha"));
    assert_eq!(item.tags, vec!["alpha", "beta"]);
    assert!(item.is_favorite);
    assert_eq!(item.session_type, "interactive");
}

#[test]
fn list_projects_and_agents_return_distinct_sorted() {
    let db = make_db();
    insert_minimal_session(&db, "s-1", "alpha", 0);
    insert_minimal_session(&db, "s-2", "beta", 1);
    insert_minimal_session(&db, "s-3", "alpha", 2); // distinct로 alpha 한 번만

    let mut projects = db.list_projects().unwrap();
    projects.sort();
    assert_eq!(projects, vec!["alpha", "beta"]);

    let agents = db.list_agents().unwrap();
    assert!(agents.contains(&"claude-code".to_string()));
}

#[test]
fn count_sessions_reflects_inserts() {
    let db = make_db();
    assert_eq!(db.count_sessions().unwrap(), 0);
    insert_minimal_session(&db, "s-1", "p", 0);
    insert_minimal_session(&db, "s-2", "p", 1);
    assert_eq!(db.count_sessions().unwrap(), 2);
}
