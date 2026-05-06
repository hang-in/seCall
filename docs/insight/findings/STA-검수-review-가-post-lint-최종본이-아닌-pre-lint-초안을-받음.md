# 검수(review)가 post-lint 최종본이 아닌 pre-lint 초안을 받음

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall/src/commands/wiki.rs:148

## Description

`wiki.rs:148` — `run_markdownlint()`가 파일을 수정할 수 있음에도, `run_review()`에 수정 전 문자열 `linked`를 그대로 전달합니다. 결과적으로 `--review` 단계는 실제 저장본이 아닌 lint 전 초안을 검수하게 되어 검수 결과의 신뢰성이 근본적으로 깨집니다.

**Evidence**: `[wiki.rs:148] `run_markdownlint()`가 파일을 수정할 수 있는데도 검수는 수정 전 문자열 `linked`를 그대로 `run_review()`에 넘깁니다. 결과적으로 `--review`는 최종 저장본이 아닌 pre-lint 초안을 검수하게 됩니다.`

## Snippet

```
// run_review(linked) — lint 적용 전 문자열 전달
```
