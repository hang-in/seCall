# SessionRepo trait에 신규 메서드 미반영

- **Category**: test
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: open
- **Note (P38, 2026-05-03)**: P38 가 `tests/session_repo_helpers.rs` (29 tests) 로 helper 호출 회귀를 보강했으나, **trait surface 자체에 신규 메서드를 추가하는 production code 변경은 본 phase 범위 외**. trait 기반 mock 테스트 격리는 여전히 미해결 → status `open` 유지. 후속 phase 에서 `SessionRepo` trait 확장 필요.
- **File**: crates/secall-core/src/store/session_repo.rs:18

## Description

SessionRepo trait은 8개 메서드만 정의하지만, Database에는 get_sessions_for_date, get_topics_for_sessions, delete_session_full, update_session_type 등 20개 이상의 메서드가 trait 외부에 직접 구현되어 있습니다. trait 기반 mock 테스트 작성이 불가능하고, 테스트 더블을 통한 단위 테스트 격리가 구조적으로 막혀 있습니다.

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
pub trait SessionRepo { /* 8개 메서드만 정의, 나머지 20+는 impl Database에 직접 구현 */ }
```
