---
type: plan
status: draft
updated_at: 2026-05-02
slug: p36-job-cancellation
version: 1
---

# P36 — Job Cancellation (실행 중 작업 취소)

## Description

P33에서 도입한 Job 시스템(`/api/commands/{sync,ingest,wiki-update}` + `/api/jobs/{id}/stream` SSE)은 실행 트리거와 진행률 표시까지만 구현. **실행 중 취소**는 `/api/jobs/{id}/cancel`이 `501 NOT_IMPLEMENTED`로 노출됨 (`crates/secall-core/src/mcp/rest.rs:667-678`).

본 phase는 tokio `CancellationToken`을 도입하여 sync/ingest/wiki-update 3개 job을 안전하게 중단 가능하게 만든다.

## 현재 한계

- `crates/secall-core/src/mcp/rest.rs:670` — api_cancel_job이 항상 501 반환.
- `crates/secall-core/src/jobs/registry.rs:18` — JobRegistry에 cancellation token 저장소 없음.
- `crates/secall-core/src/jobs/mod.rs:29-33` — ProgressSink trait에 cancel 체크 메서드 없음.
- `crates/secall/src/commands/{sync,ingest,wiki}.rs` — `run_with_progress`가 phase/loop 사이에 cancel check 안 함.
- web UI: 실행 중 job 취소 버튼 없음 (`JobBanner`, `JobItem` 둘 다).

## Expected Outcome

- `POST /api/jobs/{id}/cancel` → 200 OK + `JobState.status` 가 다음 안전 지점에서 `interrupted` 로 전이.
- `GET /api/jobs/{id}/stream` 구독자는 `{ "type": "failed", "error": "cancelled by user", "partial_result": {...} }` 이벤트 수신.
- web UI: JobBanner 와 JobItem(running 상태) 에 "취소" 버튼 + 확인 다이얼로그 → cancelJob mutation → SSE에서 interrupted 표시.
- 부분 완료 결과(예: ingest 50/100 처리 후 취소) 는 `partial_result` 필드에 보존.

## Subtasks

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 00 | CancellationToken 인프라 (registry + executor + REST) | A | — |
| 01 | Adapter 통합 (sync/ingest/wiki) — 안전 지점에 cancel check | B | 00 |
| 02 | web UI cancel 버튼 + useCancelJob mutation | A | — |
| 03 | README + CI 업데이트 | C | 00, 01, 02 |

병렬 실행 전략:
- Phase A — Task 00 + 02 동시 dispatch (백엔드 인프라 / 웹 UI 분리, 02 는 백엔드 cancel API 가 501 이어도 mutation 정의는 가능)
- Phase B — Task 01 (Task 00 의 sink.is_cancelled API 필요)
- Phase C — Task 03 (모든 task 완료 후 정확한 동작 반영)

## Constraints

- **수정 금지**: P32~35 완료 코드의 동작 변경. 본 phase는 cancellation 추가만.
- **부분 완료 보존**: ingest 진행 중 취소 → 이미 처리된 세션은 commit, 미처리 세션은 skip. partial_result에 통계 표시.
- **외부 API 호출 도중 취소**: reqwest 호출은 `tokio::select!` 또는 `CancellationToken::run_until_cancelled` 로 token 과 race — request 자체 abort 가능.
- **DB 트랜잭션 도중 취소 금지**: 트랜잭션 시작 후 commit 전까지는 cancel check 안 함 → 일관성 보장. check 는 트랜잭션 경계 사이에 둔다.
- **단일 job 단위 cancel**: bulk cancel 미지원.

## Non-goals

- 진행 중인 외부 명령(`git pull` 같은 subprocess) 취소: SIGKILL 은 안전성 낮음 → 본 phase 범위 외 (다음 phase 검토).
- 취소 후 자동 재시작 / resume: 별도 phase.
- 취소 권한 검사: 현재 로컬 전용 서버 → 미구현.
- WebSocket 기반 양방향 cancel push: 현재 SSE 단방향. POST 트리거로 충분.

## Success criteria

- 실행 중 sync/ingest/wiki-update 에 `POST /api/jobs/{id}/cancel` → 200 + 5초 이내 `status: "interrupted"` 전이.
- SSE 구독자가 `Failed { error: "cancelled by user", partial_result: ... }` 이벤트 수신.
- web UI 에서 cancel 버튼 클릭 → 확인 다이얼로그 → mutation 후 JobItem 의 상태 표시가 "interrupted" 로 갱신.
- `cargo test` 통합 테스트 1건: cancel 시나리오 (가짜 long-running adapter 를 cancel → registry status = Interrupted, partial_result 보존).
