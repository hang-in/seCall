# review 실패 시 자동 재시도 로직 없음

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall/src/commands/wiki.rs:595

## Description

`wiki.rs:595` — `approved == false` 또는 `severity=error` 이슈가 존재해도 결과를 출력만 하고 프로세스가 종료됩니다. Task 03 계약에서 요구한 '자동 수정 1회 + 재검수' 루프가 구현되지 않아 검수 단계가 단순 리포트로만 동작합니다.

**Evidence**: `[wiki.rs:595] `--review` 경로가 `approved == false`이거나 `severity=error` 이슈가 있어도 결과를 출력만 하고 종료합니다. Task 03 계약은 error급 이슈에 대해 자동 수정 1회와 재검수를 요구하는데, 그 재시도 로직이 없어 검수 단계가 단순 리포트로만 동작합니다.`

## Snippet

```
// severity=error → 출력 후 종료, 재시도 없음
```
