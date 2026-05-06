# 페이지 간 내부 링크 생성 로직 미구현

- **Category**: stability
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall-core/src/wiki/lint.rs:113

## Description

`lint.rs:113` — `insert_obsidian_links()`가 `session_ids`/`vault_paths`만 처리합니다. Task 02 계약에서 요구한 '알려진 위키 페이지 제목이 본문에 나오면 `[[페이지명]]`으로 변환' 로직이 없어 페이지 간 내부 링크가 생성되지 않습니다.

**Evidence**: `[lint.rs:113] `insert_obsidian_links()`는 `session_ids`/`vault_paths`만 받아 세션 참조만 링크화합니다. Task 02 계약에 있던 '알려진 위키 페이지 제목이 본문에 나오면 `[[페이지명]]`으로 변환' 로직이 없어서 페이지 간 내부 링크 생성이 누락됩니다.`

## Snippet

```
// insert_obsidian_links(session_ids, vault_paths) — 페이지 간 링크 변환 없음
```
