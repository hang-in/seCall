# Review Report: P37 — 시맨틱 Graph Sync 자동화 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-03 07:20
> Plan Revision: 1

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/ingest.rs:642 — ingest 경로에서 `extract_one_session_semantic()` 성공 후 `semantic_extracted_at`를 갱신하지 않습니다. 반면 rebuild 경로는 [crates/secall/src/commands/graph.rs](/Users/d9ng/privateProject/seCall/crates/secall/src/commands/graph.rs:280)에서 성공 시 timestamp를 기록합니다. 이 차이 때문에 새로 ingest 되어 시맨틱 추출에 성공한 세션도 계속 `NULL` 상태로 남고, `--retry-failed`가 정상 성공 세션까지 다시 집어오므로 P37의 상태 추적/재시도 의미가 깨집니다.
2. web/src/components/JobOptionsDialog.tsx:357 — UI가 session 입력란에 `prefix(8+자리) ID`를 지원한다고 안내하지만, 실제 backend 필터는 [crates/secall-core/src/store/session_repo.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/store/session_repo.rs:1007)의 `WHERE id = ?1` exact match만 사용합니다. 사용자는 8자리 prefix를 넣고도 0건 처리되는 오동작을 겪게 됩니다.

## Recommendations

1. `web/src/components/JobItem.tsx`도 함께 수정했으므로, task 문서의 Changed files/Scope boundary에 이 파일을 반영해 계약과 구현을 맞추는 편이 안전합니다.
2. `crates/secall-core/tests/jobs_rest.rs`의 신규 테스트는 executor 직접 호출 수준이라 실제 Axum 라우트(`/api/commands/graph-rebuild`) 배선까지는 검증하지 않습니다. route-level 테스트를 별도로 두는 것이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 v8 + state tracking (`semantic_extracted_at`) | ✅ done |
| 2 | CLI `graph rebuild` 명령 + GraphRebuildArgs/Outcome + `run_with_progress` | ✅ done |
| 3 | REST `/api/commands/graph-rebuild` + Job 어댑터 + P36 cancel 지원 | ✅ done |
| 4 | web UI: CommandsRoute 카드 + JobOptionsDialog 옵션 + types/api | ✅ done |
| 5 | README + CI 업데이트 | ✅ done |

