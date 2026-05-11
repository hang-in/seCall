# Implementation Result: P45 — Session lifecycle backbone (archive + vault SSOT + 기본 filter)

> Developer: claude
> Branch: N/A
> Date: 2026-05-12 03:44
> Plan Revision: 0

---

## Summary

모든 테스트 통과입니다.

```
Verification results for Task 05 (rework):
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core --lib search::vector` — 13 passed (신규 3개 포함)
✅ `cargo test -p secall-core --test session_archive` — 4 passed
```

**변경 요약:**

- `crates/secall-core/src/search/bm25.rs` — `SessionMeta`에 `is_archived: bool` 필드 추가
- `crates/secall-core/src/store/session_repo.rs` — `get_session_meta` SQL에 `is_archived` 컬럼 추가 + 매퍼에서 `bool` 변환
- `crates/secall-core/src/search/vector.rs` — `passes_filters` 첫 번째 체크에 `!include_archived && meta.is_archived` 추가 + 단위 테스트 3개 추가
- `crates/secall-core/src/search/hybrid.rs` — 테스트 `SessionMeta` 리터럴에 `is_archived: false` 추가

ANN 경로(line 292)와 BLOB 스캔 경로(line 327) 양쪽 모두 `passes_filters`를 통하므로 단일 수정으로 두 경로 모두 커버됩니다.

## Subtask Results

### 1. 모든 테스트 통과입니다.

```
Verification results for Task 05 (rework):
✅ `cargo check -p secall-core` — exit 0
✅ `cargo test -p secall-core --lib search::vector` — 13 passed (신규 3개 포함)
✅ `cargo test -p secall-core --test session_archive` — 4 passed
```

**변경 요약:**

- `crates/secall-core/src/search/bm25.rs` — `SessionMeta`에 `is_archived: bool` 필드 추가
- `crates/secall-core/src/store/session_repo.rs` — `get_session_meta` SQL에 `is_archived` 컬럼 추가 + 매퍼에서 `bool` 변환
- `crates/secall-core/src/search/vector.rs` — `passes_filters` 첫 번째 체크에 `!include_archived && meta.is_archived` 추가 + 단위 테스트 3개 추가
- `crates/secall-core/src/search/hybrid.rs` — 테스트 `SessionMeta` 리터럴에 `is_archived: false` 추가

ANN 경로(line 292)와 BLOB 스캔 경로(line 327) 양쪽 모두 `passes_filters`를 통하므로 단일 수정으로 두 경로 모두 커버됩니다.

