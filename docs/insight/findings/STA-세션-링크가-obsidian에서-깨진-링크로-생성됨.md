# 세션 링크가 Obsidian에서 깨진 링크로 생성됨

- **Category**: stability
- **Severity**: major
- **Fix Difficulty**: guided
- **Status**: open
- **File**: crates/secall-core/src/wiki/lint.rs:116

## Description

`lint.rs:116` — `insert_obsidian_links()`가 세션 참조를 `[[<full-session-id>|<short-id>]]` 형식으로 생성하지만 실제 vault 경로는 `sessions/{short_id}.md` 또는 `raw/sessions/YYYY-MM-DD_session-id` 계열입니다. vault 경로를 계산하지 않아 생성된 모든 세션 링크가 Obsidian에서 dead link가 됩니다.

**Evidence**: `[lint.rs:116] 세션 링크를 `[[{full_uuid}|{short_id}]]`로 생성하지만, 실제 생성 경로는 `sessions/{short_id}.md` 또는 `projects/...`라서 링크 대상 노트가 존재하지 않습니다. 프로젝트의 링크 규약은 `[[raw/sessions/YYYY-MM-DD_session-id]]` 계열입니다.`

## Snippet

```
// [[{full_uuid}|{short_id}]] — vault 경로 불일치로 dead link
```
