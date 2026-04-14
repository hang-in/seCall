# Implementation Result: P25 — Semantic Graph 활용 + Obsidian 플러그인 (Phase 0-1)

> Developer: unknown
> Branch: N/A
> Date: 2026-04-14 18:25
> Plan Revision: 0

---

## Summary

먼저 `do_get()`이 반환하는 `SessionMeta` 구조체의 필드를 확인합니다.

`SessionMeta`에 `start_time` 필드가 있습니다. 하지만 `do_get()`은 `get_session_meta()`를 호출하고 그 결과를 `serde_json::to_value()`로 직렬화합니다. Serialize가 `start_time`을 어떤 키로 내보내는지 확인합니다.

`SessionMeta`에 `Serialize` derive가 없습니다. 그런데 `do_get()`에서 `serde_json::to_value(&meta)`를 호출하고 있습니다.

`Serialize`가 없는데 `to_value()`를 호출하면 컴파일 에러가 나야 합니다. 하지만 `cargo check`가 통과했으므로... `get_session_meta`가 다른 타입을 반환하는지 확인합니다.

이것으로 확인됐습니다. `get_session_meta`가 반환하는 `SessionMeta`는 `db.rs:235`의 것이 아니라 **다른** `SessionMeta`입니다. 검색 모듈의 것을 확인합니다.

2개의 `SessionMeta`가 있습니다:

1. **`search/bm25.rs:48`** — `Serialize` 있음, `date` 필드, `vault_path` 있음, `summary` 없음
2. **`store/db.rs:235`** — `Serialize` **없음**, `start_time` 필드, `summary` 있음

`get_session_meta`는 `session_repo.rs`의 trait 구현에서 반환합니다. import를 확인합니다.

핵심 발견! `get_session_meta()`는 `search::bm25::SessionMeta`를 반환합니다. 이 구조체에는 `date` 필드가 있지 `start_time`은 없습니다. 그리고 `Serialize`가 있어서 `to_value()`가 동작합니다.

따라서 `do_get()`가 반환하는 JSON에는 `start_time`이 아니라 `date`가 있습니다. `session-view.ts`에서 `data.start_time` → `data.date`로 수정하면 됩니다.

또한 recommendation의 `basePath` 방어 코드도 함께 적용합니다.

**Finding 수정**: `data.start_time` → `data.date`

**Recommendation 적용**: `search-view.ts`의 `basePath` 방어 코드

검증합니다.

```
Verification results for Rework:
✅ `cargo check` — exit 0
✅ `cargo test` — 274 passed, 0 failed
✅ `npx tsc --noEmit --skipLibCheck` — exit 0
✅ `npm run build` — exit 0
✅ `ls -la main.js` — 5.6KB
```

## 수정 내용

### Finding 1: `session-view.ts:55` — `data.start_time` → `data.date`
- `do_get()`가 반환하는 `SessionMeta`는 `search/bm25.rs`의 것으로, `date` 필드 사용 (`start_time`은 존재하지 않음)
- `session-view.ts:55`의 메타 줄을 `data.date`로 수정

### Recommendation: `search-view.ts:64-67` — basePath 빈 문자열 방어
- `vaultRoot`가 빈 문자열일 때 `startsWith("")`가 항상 참이 되어 경로 첫 글자가 잘리는 버그 방어
- `vaultRoot && vaultPath.startsWith(vaultRoot + "/")` 조건으로 변경

## Subtask Results

### 1. 먼저 `do_get()`이 반환하는 `SessionMeta` 구조체의 필드를 확인합니다.

`SessionMeta`에 `start_time` 필드가 있습니다. 하지만 `do_get()`은 `get_session_meta()`를 호출하고 그 결과를 `serde_json::to_value()`로 직렬화합니다. Serialize가 `start_time`을 어떤 키로 내보내는지 확인합니다.

`SessionMeta`에 `Serialize` derive가 없습니다. 그런데 `do_get()`에서 `serde_json::to_value(&meta)`를 호출하고 있습니다.

`Serialize`가 없는데 `to_value()`를 호출하면 컴파일 에러가 나야 합니다. 하지만 `cargo check`가 통과했으므로... `get_session_meta`가 다른 타입을 반환하는지 확인합니다.

이것으로 확인됐습니다. `get_session_meta`가 반환하는 `SessionMeta`는 `db.r

