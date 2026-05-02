# Review Report: P36 — Job Cancellation (실행 중 작업 취소) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-03 05:58
> Plan Revision: 1

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/jobs/executor.rs:150 — cancel된 job에서 `result_json`을 무조건 `None`으로 덮어써 어댑터가 `Ok(partial_outcome)`로 반환한 부분 결과가 registry/DB에 저장되지 않습니다. plan과 Task 01의 "partial_result 보존" 계약을 위반합니다.
2. crates/secall-core/src/jobs/executor.rs:201 — cancel 최종 SSE 이벤트를 `partial_result: None`으로 고정해 스트림 구독자가 취소 시점의 부분 결과를 받을 수 없습니다. Expected Outcome의 failed event + partial_result 요구와 어긋납니다.
3. crates/secall/src/commands/wiki.rs:99 — `run_with_progress()`가 `run_update_with_sink()` 성공 시 항상 `pages_written: 0`을 반환합니다. 따라서 wiki job은 정상 완료나 취소 후에도 실제 작성된 페이지 수를 결과로 보존하지 못합니다.

## Recommendations

1. web/src/components/JobItem.tsx:145 — cancel SSE를 `failed`로만 매핑하지 말고, `cancelled by user` 또는 별도 신호를 `interrupted` 상태로 반영하면 UI 일관성이 좋아집니다.
2. web/src/hooks/useJob.ts:106 — detail invalidate 키가 현재 `useJob()`의 query key(`["jobs","detail",id]`)와 다릅니다. 현재 사용처는 없지만 추후 단건 상세 뷰에서는 갱신 누락 원인이 됩니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | CancellationToken 인프라 (registry + executor + REST) | ✅ done |
| 2 | Adapter 통합 (sync/ingest/wiki) | ✅ done |
| 3 | web UI cancel 버튼 + useCancelJob mutation | ✅ done |
| 4 | README + CI 업데이트 | ✅ done |

