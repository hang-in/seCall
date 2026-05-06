# Rework 절차에 반복 실패 방지 장치가 주석 수준으로만 존재

- **Category**: debt
- **Severity**: info
- **Fix Difficulty**: guided
- **Status**: open
- **File**: docs/agents/developer.md:63

## Description

developer.md의 Rework 섹션에서 '이전 시도 이력'을 확인하라는 지침이 있지만, 이 이력이 어디에 어떤 형식으로 저장되는지 명시되지 않습니다. 이력 미참조로 동일한 실수가 반복될 수 있습니다.

**Evidence**: `3. Check "이전 시도 이력" to avoid repeating past mistakes`

## Snippet

```
When you receive a rework request with review findings:
1. Read each finding carefully — **only fix the specified subtasks**
2. If "대상 서브태스크" is specified, do NOT modify other tasks' code
3. Check "이전 시도 이력" to avoid repeating past mistakes
```
