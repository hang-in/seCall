# is_date_dir가 유효하지 않은 날짜를 통과시킴

- **Category**: stability
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall-core/src/graph/build.rs:21

## Description

is_date_dir 함수는 YYYY-MM-DD 형식 검사만 하고, 실제 날짜 범위(월 00-99, 일 00-99)를 검증하지 않습니다. '2026-99-99' 같은 값도 true를 반환합니다. 다만 실제 파일시스템에서 이런 디렉토리가 생성될 가능성은 낮으므로 직접적인 프로덕션 장애보다는 방어적 코딩 부족에 해당합니다.

**Evidence**: `fn is_date_dir(name: &str) -> bool {
    name.len() == 10
        && name.as_bytes()[4] == b'-'
        && name.as_bytes()[7] == b'-'
        && name[..4].chars().all(|c| c.is_ascii_digit())
        && name[5..7].chars().all(|c| c.is_ascii_digit())
        && name[8..10].chars().all(|c| c.is_ascii_digit())
}
// 주석: "2026-99-99" → true (범위 불검사)`

## Snippet

```
fn is_date_dir(name: &str) -> bool { ... }
```
