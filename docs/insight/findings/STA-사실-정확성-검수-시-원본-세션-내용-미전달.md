# 사실 정확성 검수 시 원본 세션 내용 미전달

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall/src/commands/wiki.rs:463

## Description

`wiki.rs:463` — 검수 단계가 모델에 `session: <id>` 또는 `batch update` 문자열만 전달합니다. 원본 세션 요약·내용을 대조 근거로 제공하지 않아 모델이 생성된 위키 내용의 사실 정확성을 실질적으로 검증할 수 없습니다.

**Evidence**: `[wiki.rs:463] 검수 단계가 모델에 전달하는 원본 근거가 `session: <id>` 또는 `batch update` 문자열뿐입니다. Task 03은 원본 세션 요약/내용을 대조용으로 전달해야 하는데, 현재 구현으로는 사실 정확성 검수가 실질적으로 동작할 수 없습니다.`

## Snippet

```
// run_review(content, "session: <id>") — 원본 세션 데이터 없음
```
