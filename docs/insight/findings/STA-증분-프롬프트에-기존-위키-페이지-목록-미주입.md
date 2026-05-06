# 증분 프롬프트에 기존 위키 페이지 목록 미주입

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall/src/commands/wiki.rs:283

## Description

`wiki.rs:283` — `build_haiku_incremental_prompt()`가 세션 메타와 턴만 직렬화하고, 기존 위키 페이지 목록을 컨텍스트로 주입하지 않습니다. `--session` 모드에서 모델이 기존 페이지와의 병합 힌트를 받지 못해 중복 페이지 생성 또는 기존 내용 누락이 발생할 수 있습니다.

**Evidence**: `[wiki.rs:283] `build_haiku_incremental_prompt()`는 세션 메타와 턴만 직렬화하고 끝나며, Task 01 계약에 있던 '기존 위키 페이지 목록도 함께 주입' 단계가 없습니다. 그 결과 `--session` 모드에서 기존 페이지 병합 힌트를 모델에 전달하지 못합니다.`

## Snippet

```
// build_haiku_incremental_prompt(): 세션 메타+턴만 직렬화, 기존 페이지 목록 없음
```
