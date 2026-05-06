# DB 메서드 get_sessions_for_date, get_topics_for_sessions 테스트 부재

- **Category**: test
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: resolved
- **File**: crates/secall-core/src/store/db.rs:658
- **Resolved Note**: `crates/secall-core/src/store/db.rs` 의 테스트 모듈에 `test_get_sessions_for_date_filters_by_date` / `test_get_topics_for_sessions_with_edges` 가 이미 존재. Gemini PR #47 review 에서 확인.

## Description

db.rs의 테스트 모듈에서 `list_session_vault_paths`, `get_all_sessions_for_classify`, `update_session_type`은 테스트되지만, `get_sessions_for_date`(line 658)와 `get_topics_for_sessions`(line 686)는 테스트가 없습니다. 이 두 메서드는 `log.rs`의 핵심 데이터 소스이며, 날짜 필터링 SQL과 그래프 조인 쿼리의 정확성을 검증하지 않으면 log 커맨드 전체가 잘못된 결과를 반환할 수 있습니다.

**Evidence**: `pub fn get_sessions_for_date(
    ...
pub fn get_topics_for_sessions(&self, session_ids: &[String]) -> Result<Vec<(String, String)>> {`

## Snippet

```
// line 658: get_sessions_for_date
// line 686: get_topics_for_sessions
// 둘 다 #[cfg(test)] 모듈에서 호출되지 않음
```
