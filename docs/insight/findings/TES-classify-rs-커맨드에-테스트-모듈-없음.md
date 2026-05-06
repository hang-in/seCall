# classify.rs 커맨드에 테스트 모듈 없음

- **Category**: test
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall/src/commands/classify.rs:55

## Description

run_backfill의 핵심 분류 로직(apply_classification)은 ingest.rs 테스트 9개로 잘 커버되어 있습니다. 그러나 classify.rs 자체의 dry_run 분기(eprintln만 하고 DB 미갱신), regex 컴파일 실패 시 조기 반환, updated 카운트 집계 로직은 별도 테스트가 없습니다. dry_run=true일 때 DB가 실제로 변경되지 않는다는 보장을 코드로 검증할 수 없습니다.

**Evidence**: `if dry_run {
    eprintln!("  [dry-run] {} → {}", short_id, new_type);
} else {
    db.update_session_type(session_id, &new_type)?;
    tracing::debug!(session = short_id, session_type = new_type, "classified");
}
updated += 1;`

## Snippet

```
// #[cfg(test)] 모듈 없음 (72줄 전체)
```
