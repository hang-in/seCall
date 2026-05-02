# Review Report: P33 — secall-web Phase 1 (명령 트리거 + SSE + Job 시스템) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-02 19:20
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/jobs/executor.rs:67 — `current_active_kind()` 체크와 `registry.register()` 사이에 원자적 보호가 없어, 동시 POST 요청 2개가 모두 idle 상태를 보고 각각 job을 등록할 수 있습니다. 단일 큐/409 Conflict 보장이 깨지는 런타임 결함입니다.
2. crates/secall-core/src/jobs/adapters/sync_adapter.rs — Task 02의 Changed files에 명시된 신규 파일이 실제로 존재하지 않습니다. `ingest_adapter.rs`, `wiki_adapter.rs`도 동일하게 누락되어 subtask 계약이 충족되지 않았습니다.
3. web/src/components/JobToastListener.tsx — Task 06의 Changed files에 명시된 신규 파일이 실제로 존재하지 않습니다. 현재는 `useJobLifecycle` 훅으로 기능 일부를 대체했지만, task 계약상 required file was not changed에 해당합니다.

## Recommendations

1. `JobExecutor`에 spawn 전용 mutex 또는 compare-and-register 단계를 추가해, active 체크와 등록을 하나의 원자 구간으로 묶으세요.
2. Task 02/06은 구현 구조를 바꿀 의도였다면 plan/task 문서를 먼저 갱신하거나, 아니면 문서에 적힌 파일 경계대로 코드를 분리해 reviewer contract와 구현을 일치시키세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 v6 | ✅ done |
| 2 | `Job` 코어 모듈 | ✅ done |
| 3 | Job → 명령 어댑터 | ✅ done |
| 4 | REST 엔드포인트 | ✅ done |
| 5 | Wiki 본문 fetch 엔드포인트 + UI | ✅ done |
| 6 | Web UI | ✅ done |
| 7 | Web UI | ✅ done |
| 8 | ingest 후 graph 자동 증분 (옵션) | ✅ done |
| 9 | README + CI | ✅ done |

