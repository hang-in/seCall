# Review Report: P38 — 테스트 갭 메우기 (REST 라우트 + session_repo) — Round 1

> Verdict: conditional
> Reviewer: 
> Date: 2026-05-03 08:48
> Plan Revision: 1

---

## Verdict

**conditional**

## Findings

1. crates/secall-core/tests/rest_routes.rs:317 — `/api/sessions` 테스트가 `project`, `agent`, `page`, `page_size`만 검증하고, Task 01에서 요구한 `tag`, `tags`, `favorite`, `since` 쿼리 파라미터 회귀를 다루지 않아 해당 필터 회귀를 잡지 못합니다.
2. crates/secall-core/tests/rest_routes.rs:403 — `/api/tags?with_counts={true|false}` 분기 테스트가 두 경우 모두 빈 배열의 “array 타입”만 확인합니다. `with_counts=true`의 `{name,count}` 객체 배열과 `with_counts=false`의 문자열 배열 차이를 실제 데이터로 검증하지 않아 Task 01의 응답 형태 계약을 충족하지 못합니다.
3. crates/secall-core/tests/session_repo_helpers.rs:174 — `list_sessions_filtered` 회귀 묶음에 Task 03이 요구한 `since` 필터 테스트가 없습니다. 현재는 `project/agent/tag/tags/favorite/page`만 검증되어 날짜 기준 필터 회귀가 비어 있습니다.
4. docs/insight/findings/TES-session_repo-rs-trait에-신규-메서드-미반영.md:6 — 이 finding은 `SessionRepo` trait의 메서드 누락이라는 구조 문제를 지적하는데, production code인 `crates/secall-core/src/store/session_repo.rs`는 이번 phase에서 수정되지 않았습니다. 그런데 문서에서는 `Status: resolved`와 `Resolved By: tests/session_repo_helpers.rs`로 바뀌어 사실과 맞지 않습니다.
5. docs/insight/findings/TES-sessionrepo-trait에-신규-메서드-미반영.md:6 — 위와 동일하게 trait surface 미반영 문제는 그대로인데 테스트 추가만으로 `resolved` 처리되어 Insight 상태가 부정확합니다.

## Recommendations

1. Task 01 재작업 시 `/api/sessions`에 `tag`, `tags`, `favorite`, `since`를 실제로 조합한 route-level 케이스를 추가하고, `/api/tags`는 태그 데이터를 넣은 뒤 `with_counts=true/false`의 payload shape 차이를 명시적으로 검증하세요.
2. Task 03 재작업 시 `SessionListFilter.since` 단독 또는 조합 케이스를 하나 추가해 날짜 기준 필터 contract를 고정하세요.
3. Task 04는 trait 관련 finding 2건을 `resolved` 대신 `open` 유지 또는 `partially addressed` 성격의 메모로 되돌리는 편이 정확합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | axum Router 통합 테스트 인프라 (test helper + dev-dep) | ✅ done |
| 2 | REST 라우트 회귀 | ✅ done |
| 3 | REST 라우트 회귀 | ✅ done |
| 4 | `session_repo` helper 회귀 통합 | ✅ done |
| 5 | README 회귀 안전망 안내 + Insight findings 해결 표시 | ✅ done |

