# Implementation Result: P36 — Job Cancellation (실행 중 작업 취소)

> Developer: unknown
> Branch: N/A
> Date: 2026-05-03 06:18
> Plan Revision: 1

---

## Summary

## Rework 완료 — 3건 모두 해결

**수정 내역:**

| # | 파일 | 변경 |
|---|---|---|
| 1 | `executor.rs:150` | `was_cancelled` 분기에서 `result.as_ref().ok().cloned()`으로 어댑터 partial_outcome 보존 → registry/DB 양쪽에 저장 |
| 2 | `executor.rs:201` | final SSE event `partial_result: result_json.clone()` — 위에서 보존된 부분 결과를 구독자에게 전달 |
| 3 | `wiki.rs` | `run_update_with_sink` 시그니처 `Result<()>` → `Result<usize>` (페이지 카운트 반환), 11개 `return Ok(())` → `return Ok(pages_written)`, 마지막 `Ok(())` → `Ok(pages_written)`, 두 `std::fs::write` 성공 후 `pages_written += 1` (haiku batch + haiku incremental 첫 작성, regen은 덮어쓰기라 카운트 안 함). `run_update` wrapper에 `.map(|_| ())` 추가로 CLI API 보존. `run_with_progress`의 outcome match에서 `Ok(pages_written)` 패턴으로 `WikiOutcome.pages_written`에 반영 |

**Verification:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — 0 warnings
✅ cargo fmt --all -- --check — exit 0
✅ cargo test jobs::executor::tests::cancel — 2 passed
✅ cargo test --test rest_listing — 12 passed (회귀)
✅ cargo test --all — 전체 통과 (doc-tests 3 ignored, 0 failed)
```

**Recommendations 처리:**
- 두 권고(JobItem cancel SSE → interrupted UI 매핑, useJob detail invalidate 키 정리)는 본 rework 범위 밖(Task 02 = web UI). 별도 phase에서 정리 권장.

**Scope 확인:** Task 02 (web), Task 03 (README) 코드는 건드리지 않음.

## Subtask Results

### 1. ## Rework 완료 — 3건 모두 해결

**수정 내역:**

| # | 파일 | 변경 |
|---|---|---|
| 1 | `executor.rs:150` | `was_cancelled` 분기에서 `result.as_ref().ok().cloned()`으로 어댑터 partial_outcome 보존 → registry/DB 양쪽에 저장 |
| 2 | `executor.rs:201` | final SSE event `partial_result: result_json.clone()` — 위에서 보존된 부분 결과를 구독자에게 전달 |
| 3 | `wiki.rs` | `run_update_with_sink` 시그니처 `Result<()>` → `Result<usize>` (페이지 카운트 반환), 11개 `return Ok(())` → `return Ok(pages_written)`, 마지막 `Ok(())` → `Ok(pages_written)`, 두 `std::fs::write` 성공 후 `pages_written += 1` (haiku batch + haiku incremental 첫 작성, regen은 덮어쓰기라 카운트 안 함). `run_update` wrapper에 `.map(|_| ())` 추가로 CLI API 보존. `run_with_progress`의 outcome match에서 `Ok(pages_written)` 패턴으로 `WikiOutcome.pages_written`에 반영 |

**Verification:**
```
✅ cargo check --all-targets — exit 0
✅ cargo clippy --all-targets --all-features — 0 warnings
✅ cargo fmt --all -- --check — exit 0
✅ cargo test jobs::executor::tests::cancel — 2 passed
✅ cargo test --test rest_listing — 12 passed (회귀)
✅ cargo test --all — 전체 통과 (doc-tests 3 ignored, 0 failed)
```

**Recommendations 처리:**
- 두 권고(JobItem cancel SSE → interrupted UI 매핑, useJob detail invalidate 키 정리)는 본 rework 범위 밖(Task 02 = web UI). 별도 phase에서 정리 권장.

**Scope 확인:** Task 02 (web), Task 03 (README) 코드는 건드리지 않음.

