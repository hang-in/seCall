# 배치 모드: 파일 생성·병합 파이프라인 전체 누락

- **Category**: stability
- **Severity**: critical
- **Fix Difficulty**: guided
- **Status**: resolved
- **File**: crates/secall/src/commands/wiki.rs:95
- **Resolved Note**: `crates/secall/src/commands/wiki.rs` 의 배치 모드 (라인 250-400 부근) 에 파일 생성 + 병합 파이프라인이 이미 구현돼 있음. Gemini PR #47 review 에서 확인 — finding 의 117/415 라인 번호도 코드 이동으로 outdated.

## Description

`wiki.rs:95` — `--session` 없이 배치 실행 시 생성 결과를 `println!`으로만 출력하고 종료합니다. `validate_frontmatter()`, `merge_with_existing()`, `insert_obsidian_links()`, 파일 쓰기, `run_markdownlint()` 호출이 전혀 없어 `wiki/` 파일이 실제로 생성되지 않습니다. 또한 `wiki.rs:117/415` — 배치 출력 전체를 단일 `index.md`로만 기록해 Task 01/02 계약의 '프로젝트별 별도 페이지 생성·병합' 요구를 충족하지 못합니다. 프로덕션에서 배치 실행 결과가 디스크에 저장되지 않는 데이터 유실 수준의 버그입니다.

**Evidence**: `[wiki.rs:95] 생성 결과를 `println!`으로만 출력하고 종료합니다. Task 02 계약상 필수인 `validate_frontmatter()`, `merge_with_existing()`, `insert_obsidian_links()`, 파일 쓰기, `run_markdownlint()` 호출이 전혀 없어 `wiki/` 파일 생성·병합이 수행되지 않습니다.
[wiki.rs:117, wiki.rs:415] 배치 모드에서 생성 결과 전체에 대해 단일 `page_path`만 계산하고, `session == None`이면 항상 `index.md`로 기록합니다.`

## Snippet

```
// wiki.rs:95 — println! 출력 후 파이프라인 종료
// wiki.rs:117/415 — session == None → page_path = index.md (단일 파일)
```
