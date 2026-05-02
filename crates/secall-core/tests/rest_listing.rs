//! P32 Task 02 — 신규 REST 엔드포인트의 통합 테스트.
//!
//! axum 라우터/HTTP 레이어를 거치지 않고 `Database`의 listing/mutation 메서드와
//! 태그 정규화를 외부 (tests/) 크레이트에서 검증한다. SearchEngine을 띄우지
//! 않아도 되는 가벼운 형태로, REST 엔드포인트가 호출하는 동일한 핵심 로직을
//! 점검한다.

use chrono::TimeZone;
use secall_core::ingest::{AgentKind, Session, TokenUsage};
use secall_core::store::session_repo::SessionListFilter;
use secall_core::store::{normalize_tag, normalize_tags, Database, SessionRepo};

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

fn default_filter() -> SessionListFilter {
    SessionListFilter {
        page: 1,
        page_size: 30,
        ..Default::default()
    }
}

#[test]
fn rest_list_sessions_paginates_and_filters_by_project() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-rust-0", "rust-proj", 0))
        .unwrap();
    db.insert_session(&make_session("s-rust-1", "rust-proj", 1))
        .unwrap();
    db.insert_session(&make_session("s-other-0", "other-proj", 2))
        .unwrap();

    // 프로젝트 필터
    let mut f = default_filter();
    f.project = Some("rust-proj".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 2);
    assert!(page
        .items
        .iter()
        .all(|i| i.project.as_deref() == Some("rust-proj")));

    // ORDER BY start_time DESC — 최근(s-rust-1)이 첫
    assert_eq!(page.items[0].id, "s-rust-1");

    // 페이지네이션
    let mut f2 = default_filter();
    f2.page_size = 1;
    let p1 = db.list_sessions_filtered(&f2).unwrap();
    assert_eq!(p1.total, 3);
    assert_eq!(p1.items.len(), 1);
    f2.page = 3;
    let p3 = db.list_sessions_filtered(&f2).unwrap();
    assert_eq!(p3.items.len(), 1);
}

#[test]
fn rest_list_sessions_excludes_automated() {
    let db = Database::open_memory().unwrap();
    let mut auto_sess = make_session("s-auto", "p", 0);
    auto_sess.session_type = "automated".to_string();
    db.insert_session(&auto_sess).unwrap();
    db.insert_session(&make_session("s-inter", "p", 1)).unwrap();

    let page = db.list_sessions_filtered(&default_filter()).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, "s-inter");
}

#[test]
fn rest_set_tags_normalizes_and_dedups() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-1", "p", 0)).unwrap();

    let normalized = db
        .update_session_tags("s-1", &["Rust".into(), "RUST".into(), "hello world".into()])
        .unwrap();
    // BTreeSet 정렬 + dedup + 정규화
    assert_eq!(normalized, vec!["hello-world", "rust"]);

    // 태그 필터로 다시 매칭됨
    let mut f = default_filter();
    f.tag = Some("rust".to_string());
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].tags, vec!["hello-world", "rust"]);
}

#[test]
fn rest_list_sessions_multi_tag_and() {
    // P34 Task 03: SessionListFilter.tags(Vec<String>) — 모두 가진 세션만 매칭.
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-both", "p", 0)).unwrap();
    db.insert_session(&make_session("s-only-rust", "p", 1))
        .unwrap();
    db.insert_session(&make_session("s-only-search", "p", 2))
        .unwrap();

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
fn rest_set_tags_returns_error_for_missing_session() {
    let db = Database::open_memory().unwrap();
    let res = db.update_session_tags("missing", &["x".into()]);
    assert!(res.is_err());
}

#[test]
fn rest_set_favorite_toggles_and_filters() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-fav", "p", 0)).unwrap();

    // 기본 false
    let mut f = default_filter();
    f.favorite = Some(true);
    assert_eq!(db.list_sessions_filtered(&f).unwrap().total, 0);

    db.update_session_favorite("s-fav", true).unwrap();
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert!(page.items[0].is_favorite);

    db.update_session_favorite("s-fav", false).unwrap();
    f.favorite = Some(false);
    let page = db.list_sessions_filtered(&f).unwrap();
    assert_eq!(page.total, 1);
    assert!(!page.items[0].is_favorite);
}

#[test]
fn rest_set_favorite_returns_error_for_missing_session() {
    let db = Database::open_memory().unwrap();
    let res = db.update_session_favorite("missing", true);
    assert!(res.is_err());
}

#[test]
fn rest_get_session_list_item_returns_meta_for_do_get() {
    // P32 Task 06 rework: do_get() 응답에 tags/is_favorite를 포함시키려면
    // 단일 세션의 SessionListItem 조회 메서드가 정확히 동작해야 한다.
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-meta", "p", 0)).unwrap();
    db.update_session_tags("s-meta", &["alpha".into(), "Beta".into()])
        .unwrap();
    db.update_session_favorite("s-meta", true).unwrap();

    let item = db.get_session_list_item("s-meta").unwrap();
    assert_eq!(item.id, "s-meta");
    assert_eq!(item.tags, vec!["alpha", "beta"]);
    assert!(item.is_favorite);
    assert_eq!(item.project.as_deref(), Some("p"));
}

#[test]
fn rest_list_projects_and_agents() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-1", "alpha", 0)).unwrap();
    db.insert_session(&make_session("s-2", "beta", 1)).unwrap();

    let mut projects = db.list_projects().unwrap();
    projects.sort();
    assert_eq!(projects, vec!["alpha", "beta"]);

    let agents = db.list_agents().unwrap();
    assert!(agents.contains(&"claude-code".to_string()));
}

#[test]
fn tag_normalize_helpers_match_rest_endpoint_behavior() {
    // PATCH /api/sessions/:id/tags가 사용하는 정규화 규칙이 외부 contract.
    assert_eq!(normalize_tag("Rust"), "rust");
    assert_eq!(normalize_tag("hello world"), "hello-world");
    assert_eq!(
        normalize_tags(&["A".into(), "a".into(), "B".into()]),
        vec!["a", "b"]
    );
}
