# session_repo.rs trait에 신규 메서드 미반영

- **Category**: test
- **Severity**: info
- **Fix Difficulty**: guided
- **Status**: open
- **Note (P38, 2026-05-03)**: P38 가 `tests/session_repo_helpers.rs` (29 tests) 로 helper 호출 회귀를 보강했으나, **trait surface 자체에 신규 메서드를 추가하는 production code 변경은 본 phase 범위 외**. trait 기반 mock 테스트 격리는 여전히 미해결 → status `open` 유지. 후속 phase 에서 `SessionRepo` trait 확장 (구조적 fix) 필요.
- **File**: crates/secall-core/src/store/session_repo.rs:5

## Description

SessionRepo trait은 8개의 메서드를 정의하지만, db.rs에서 직접 `impl Database`로 추가된 `get_sessions_for_date`, `get_topics_for_sessions`, `update_session_type`, `get_all_sessions_for_classify` 등은 trait에 포함되지 않습니다. trait 기반 mock 테스팅이 불가능하여 커맨드 레이어의 단위 테스트 작성이 어렵습니다.

**Evidence**: `pub trait SessionRepo {
    fn insert_session(&self, session: &Session) -> Result<()>;
    fn update_session_vault_path(&self, session_id: &str, vault_path: &str) -> Result<()>;
    fn insert_turn(&self, session_id: &str, turn: &Turn) -> Result<i64>;
    fn session_exists(&self, session_id: &str) -> Result<bool>;
    fn session_exists_by_prefix(&self, prefix: &str) -> Result<bool>;
    fn get_session_meta(&self, session_id: &str) -> Result<SessionMeta>;
    fn is_session_open(&self, session_id: &str) -> Result<bool>;
    fn delete_session(&self, session_id: &str) -> Result<()>;
}`

## Snippet

```
pub trait SessionRepo { ... } // 8개 메서드만 정의, DB에 추가된 4+ 메서드 미포함
```
