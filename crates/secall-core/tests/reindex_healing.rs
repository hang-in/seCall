//! reindex healing 불변식 회귀 테스트.
//!
//! 실제 CLI orchestration(`secall reindex --repair-missing-turns`)이 사용하는 core
//! 프리미티브(insert_session_from_vault → clear_session_fts → insert_turn/insert_fts)
//! 의 조합을 직접 검증한다. zero-turn 세션 복구, 정상 세션 no-op, 재실행 무중복.

use secall_core::ingest::markdown::{parse_session_turns, SessionFrontmatter};
use secall_core::store::{Database, SearchRepo, SessionRepo};

fn zero_turn_frontmatter(id: &str, turns: u32) -> SessionFrontmatter {
    SessionFrontmatter {
        session_id: id.to_string(),
        agent: "claude-code".to_string(),
        start_time: "2026-04-01T00:00:00Z".to_string(),
        turns: Some(turns),
        ..Default::default()
    }
}

/// insert_session_from_vault 는 turns 를 저장하지 않아 zero-turn 세션이 된다.
/// (현 운영 DB 상태 재현 — healing 대상)
fn seed_zero_turn(db: &Database, id: &str) {
    let fm = zero_turn_frontmatter(id, 2);
    db.insert_session_from_vault(&fm, "some body text", "raw\\sessions\\2026-04-01\\x.md")
        .unwrap();
}

/// CLI heal_session 과 동일한 순서로 turns 를 복구한다.
fn heal(db: &Database, id: &str, md: &str, real_path: &str) -> usize {
    let turns = parse_session_turns(md).unwrap();
    db.clear_session_fts(id).unwrap();
    for turn in &turns {
        // 테스트에서는 토큰화 대신 raw content 를 FTS 에 넣는다(불변식 검증엔 무관).
        db.insert_turn(id, turn).unwrap();
        db.insert_fts(&turn.content, id, turn.index).unwrap();
    }
    db.update_session_vault_path(id, real_path).unwrap();
    turns.len()
}

const SESSION_MD: &str =
    "---\nsession_id: s1\n---\n\n## Turn 1 — User\n\nquestion\n\n## Turn 2 — Assistant\n\nanswer\n";

#[test]
fn zero_turn_session_is_healed_from_vault() {
    let db = Database::open_memory().unwrap();
    seed_zero_turn(&db, "s1");

    assert_eq!(db.count_session_turns("s1").unwrap(), 0, "starts zero-turn");
    assert_eq!(db.count_zero_turn_sessions().unwrap(), 1);
    // insert_session_from_vault 는 turn_id=0 단일 FTS 블롭 1개를 남긴다.
    assert_eq!(db.count_fts_rows().unwrap(), 1, "legacy single FTS blob");

    let n = heal(&db, "s1", SESSION_MD, "raw/.sessions/2026-04-01/s1.md");
    assert_eq!(n, 2);
    assert_eq!(db.count_session_turns("s1").unwrap(), 2, "turns recovered");
    assert_eq!(db.count_zero_turn_sessions().unwrap(), 0);
    // legacy 블롭 제거 + per-turn 2개 → FTS 총 2개 (중복 없음)
    assert_eq!(
        db.count_fts_rows().unwrap(),
        2,
        "legacy blob replaced by per-turn"
    );
}

#[test]
fn healing_is_idempotent_on_reruns() {
    let db = Database::open_memory().unwrap();
    seed_zero_turn(&db, "s1");

    heal(&db, "s1", SESSION_MD, "raw/.sessions/2026-04-01/s1.md");
    // 재실행 — turns(OR IGNORE) 와 FTS(clear 후 재삽입) 모두 중복 누적되지 않아야 함
    heal(&db, "s1", SESSION_MD, "raw/.sessions/2026-04-01/s1.md");

    assert_eq!(
        db.count_session_turns("s1").unwrap(),
        2,
        "no duplicate turns"
    );
    assert_eq!(db.count_fts_rows().unwrap(), 2, "no duplicate FTS rows");
}

#[test]
fn healthy_session_turns_are_preserved() {
    // 이미 turns 가 있는 세션은 healing 대상이 아님(reindex 는 count>0 이면 skip).
    // 여기서는 재삽입이 OR IGNORE 로 기존 turns 를 늘리지 않음을 확인.
    let db = Database::open_memory().unwrap();
    seed_zero_turn(&db, "s1");
    heal(&db, "s1", SESSION_MD, "raw/.sessions/2026-04-01/s1.md");
    assert_eq!(db.count_session_turns("s1").unwrap(), 2);

    // 동일 turns 재삽입 시도 → 개수 불변
    let turns = parse_session_turns(SESSION_MD).unwrap();
    for turn in &turns {
        db.insert_turn("s1", turn).unwrap();
    }
    assert_eq!(
        db.count_session_turns("s1").unwrap(),
        2,
        "OR IGNORE keeps existing turns"
    );
}
