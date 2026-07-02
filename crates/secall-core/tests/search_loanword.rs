//! 외래어 음역 alias + OR/prefix 질의 확장의 end-to-end 회귀 테스트.
//!
//! 실제 FTS5 를 통해, 한국어 음역 질의("리프레시")가 영어 원어("refresh")로 색인된
//! 발언을 매칭하는지(교차스크립트 갭 해소) + 다토큰 질의가 OR 로 리콜을 확보하는지 검증.

use secall_core::ingest::markdown::SessionFrontmatter;
use secall_core::search::bm25::SearchFilters;
use secall_core::search::tokenizer::{SimpleTokenizer, Tokenizer};
use secall_core::store::{Database, SearchRepo};

/// 세션 row(JOIN 대상) + 영어 원어로 색인된 FTS 행 1개를 심는다.
fn seed_english_session(db: &Database, id: &str, content: &str) {
    let fm = SessionFrontmatter {
        session_id: id.to_string(),
        agent: "claude-code".to_string(),
        start_time: "2026-07-01T00:00:00Z".to_string(),
        ..Default::default()
    };
    // body 는 비워 turn_id=0 블롭이 생기지 않게 하고, FTS 는 아래에서 직접 넣는다.
    db.insert_session_from_vault(&fm, "", &format!("raw/.sessions/{id}.md"))
        .unwrap();
    // 색인 동작 모사: tokenize_for_fts (형태소/공백 조인). 영어는 원어 그대로 색인됨.
    let indexed = SimpleTokenizer.tokenize_for_fts(content);
    db.insert_fts(&indexed, id, 0).unwrap();
}

#[test]
fn korean_loanword_query_matches_english_content() {
    let db = Database::open_memory().unwrap();
    seed_english_session(&db, "s1", "we refresh the auth token stored in keychain");
    let filters = SearchFilters::default();

    // 한글 음역 질의 → alias(refresh/token) 병기로 영어 발언 매칭
    let q = SimpleTokenizer.fts_query("리프레시 토큰");
    let hits = db.search_fts(&q, 10, &filters).unwrap();
    assert_eq!(
        hits.len(),
        1,
        "loanword alias 로 영어 발언이 매칭돼야: q={q}"
    );

    // 대조: alias 없는 순수 한글 prefix 는 교차스크립트 매칭 실패(해소하려던 갭 재현)
    let no_alias = db.search_fts("리프레시*", 10, &filters).unwrap();
    assert!(
        no_alias.is_empty(),
        "alias 없으면 한글 질의가 영어 발언을 못 잇는다"
    );
}

#[test]
fn multi_token_query_uses_or_not_implicit_and() {
    let db = Database::open_memory().unwrap();
    seed_english_session(&db, "s1", "we refresh the auth token stored in keychain");
    let filters = SearchFilters::default();

    // "refresh" 는 있고 "database" 는 없는 다토큰 질의.
    // OR 확장이면 refresh 로 매칭, 암묵적 AND 였다면 0건.
    let q = SimpleTokenizer.fts_query("리프레시 데이터베이스");
    assert!(q.contains(" OR "), "다토큰 질의는 OR 로 조인: {q}");
    let hits = db.search_fts(&q, 10, &filters).unwrap();
    assert_eq!(hits.len(), 1, "부분 매치도 OR 로 리콜 확보: q={q}");
}
