# 신규 log.rs 커맨드에 테스트 모듈 없음 (200 LOC)

- **Category**: test
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall/src/commands/log.rs:29

## Description

새로 추가된 `log.rs`는 200줄의 복잡한 워크플로우(DB 조회, 세션 필터링, 프로젝트별 그룹핑, Ollama API 호출, 파일 I/O)를 포함하지만 `#[cfg(test)]` 모듈이 전혀 없습니다. `meaningful` 필터링 로직(turns >= 2, stype != "automated"), 노이즈 스킵 조건(starts_with 3가지), `generate_template` 함수 등 순수 로직 부분은 단위 테스트가 가능하며, 이 없이는 필터링 조건 변경 시 regression 위험이 높습니다.

**Evidence**: `sessions.iter().filter(|(_, _, _, turns, _, stype)| { *turns >= 2 && stype != "automated" })`

## Snippet

```
let meaningful: Vec<_> = sessions
    .iter()
    .filter(|(_, _, _, turns, _, stype)| {
        *turns >= 2 && stype != "automated"
    })
    .collect();
```
