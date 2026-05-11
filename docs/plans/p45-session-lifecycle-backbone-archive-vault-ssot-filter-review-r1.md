# Review Report: P45 — Session lifecycle backbone (archive + vault SSOT + 기본 filter) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-12 03:38
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/vector.rs:327 — archived 필터가 vector 검색 경로에 적용되지 않습니다. `db.search_vectors(...)`가 archive 상태를 고려하지 않은 row를 가져오고, 이후 `passes_filters`도 `is_archived`를 검사하지 않아 archived 세션이 vector-only 검색에 그대로 노출됩니다. 이 값은 [crates/secall-core/src/search/hybrid.rs:163]의 vector-only 분기와 RRF 결합 경로로 그대로 전달되므로, Task 05의 “BM25 / hybrid / vector 에 archived 제외” 요구를 충족하지 못합니다.

## Recommendations

1. `turn_vectors` 조회 단계에서 `sessions`와 JOIN하여 `sessions.is_archived = 0`를 적용하거나, vector 메타 조회 경로에 archived 상태를 포함해 `passes_filters`에서 차단하세요.
2. `crates/secall-core/src/store/session_repo.rs` 테스트 모듈에 Task 04 문서가 요구한 `test_archive_session_vault_write_fails_rolls_back_db`를 추가해 vault write 실패 시 rollback 경로를 고정하세요.
3. `crates/secall-core/tests/session_archive.rs`에 vector-only 또는 hybrid 경로에서 archived 세션이 제외되는 회귀 테스트를 추가하세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB migration (schema v10) | ✅ done |
| 2 | Vault frontmatter writer 확장 | ✅ done |
| 3 | Ingest frontmatter parser 확장 | ✅ done |
| 4 | Store archive_session/restore_session | ✅ done |
| 5 | 기본 list/search/recall 에 archive filter | ✅ done |
| 6 | 회귀 테스트 | ✅ done |

