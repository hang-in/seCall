# 세션 ID 중복 등장 시 첫 번째 이후 링크 치환 누락

- **Category**: stability
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall-core/src/wiki/lint.rs:121

## Description

`lint.rs:121` — 같은 세션 ID가 이미 한 번 링크된 경우 나머지 평문 참조를 건너뜁니다. 같은 세션이 문서 내 여러 위치에 언급되면 일부가 링크되지 않은 채로 저장됩니다.

**Evidence**: `[lint.rs:121] 같은 세션 ID가 이미 한 번 링크돼 있으면 그 세션의 나머지 평문 참조까지 전부 건너뛰고, 그렇지 않은 경우에도 첫 번째 매치만 치환합니다. 한 문서에 같은 세션 ID가 여러 번 나오면 일부 참조가 평문으로 남습니다.`

## Snippet

```
// 첫 번째 매치만 치환, 이후 동일 ID 평문 잔존
```
