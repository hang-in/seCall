---
type: plan
status: draft
updated_at: 2026-05-02
slug: p37-graph-sync
version: 1
---

# P37 — 시맨틱 Graph Sync 자동화

## Description

P26 (Gemini API 백엔드) 으로 시맨틱 엣지 추출 자체는 가능해졌지만, 현재 시맨틱 그래프 추출은 **`ingest::run` 안에 인라인** 으로 새 세션에 대해서만 처리됨 (`crates/secall/src/commands/ingest.rs:605-700`). 이미 ingest 된 세션의 시맨틱 그래프를 나중에 재계산할 수단이 없고, 모델/프롬프트 변경 시 전체 재구축 / 부분 재시도 수단도 부재. 처리 상태(완료/실패/미처리) 가시성도 없다.

본 phase 는 시맨틱 그래프 sync 를 **독립 명령** 으로 분리하여 (1) 처리 상태 추적, (2) CLI / REST 양쪽 트리거, (3) Job 시스템 (P33) 통합으로 진행률 + 취소 (P36) 지원, (4) web UI 에서 실행 가능하게 만든다.

## 현재 한계

- `crates/secall/src/commands/ingest.rs:605-700` — 새 세션만, ingest 시점에만 처리.
- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION = 7`, sessions 테이블에 시맨틱 처리 상태 컬럼 없음.
- `crates/secall/src/main.rs:347-359` — `secall graph` 서브커맨드는 build/stats/export 만, rebuild/sync 없음.
- `crates/secall-core/src/mcp/rest.rs:150-152` — REST commands 는 sync/ingest/wiki-update 3개만.
- web UI: `web/src/components/CommandButton.tsx` 카드 3개 (sync/ingest/wiki) — graph 트리거 부재.
- 실패 세션 자동 재시도 로직 부재 → 모델 일시 장애 시 사람이 직접 reingest.

## Expected Outcome

- DB 스키마 v8: `sessions.semantic_extracted_at: Option<i64>` (NULL = 미처리, Unix epoch = 마지막 성공 시각). 마이그레이션은 NULL 초기화 → 기존 동작 유지.
- CLI: `secall graph rebuild [--since DATE] [--session ID] [--all] [--retry-failed]`.
- REST: `POST /api/commands/graph-rebuild` (P33 Job 시스템 패턴 + P36 cancel 지원).
- web UI: CommandsRoute 에 "graph rebuild" 카드 + JobOptionsDialog 옵션 (since / session / all / retry-failed).
- 진행률: 세션 단위 progress (`sink.progress(i / total)`). 안전 지점 cancel 폴링.
- Outcome: `GraphRebuildOutcome { processed, succeeded, failed, skipped, edges_added }` 통계 노출.

## Subtasks

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 00 | DB 스키마 v8 + state tracking (`semantic_extracted_at`) | A | — |
| 01 | CLI `graph rebuild` 명령 + GraphRebuildArgs/Outcome + `run_with_progress` | B | 00 |
| 02 | REST `/api/commands/graph-rebuild` + Job 어댑터 + P36 cancel 지원 | B | 01 |
| 03 | web UI — CommandsRoute 카드 + JobOptionsDialog 옵션 + types/api | C | 02 |
| 04 | README + CI 업데이트 | D | 00, 01, 02, 03 |

병렬 실행 전략:
- Phase A — Task 00 (DB 단독)
- Phase B — Task 01 → Task 02 순차 (02 가 01 의 `run_with_progress` 시그니처 의존)
- Phase C — Task 03 (web)
- Phase D — Task 04 (README)

## Constraints

- **수정 금지**: P32~36 완료 코드 동작 변경. 본 phase 는 추가만.
- **기존 ingest 동작 유지**: ingest 시점 시맨틱 추출은 그대로. `semantic_extracted_at` 도 ingest 성공 시 set.
- **단일 큐 정책 (P33)**: graph rebuild 도 sync/ingest/wiki 와 동일 큐 — 동시 실행 1개만.
- **P36 cancel**: graph rebuild 도 안전 지점 (세션 루프 시작 + LLM 호출 직전) 에서 폴링.
- **DB 트랜잭션 도중 cancel 금지**: 단일 세션 처리 (extract + insert) 는 원자 단위.
- **partial_outcome 보존**: cancel 시 `Ok(partial_outcome)` 반환.

## Non-goals

- 시맨틱 추출 로직 자체 변경 (`crates/secall-core/src/graph/semantic/`) — 본 phase 는 sync 자동화만.
- 시맨틱 모델 결과 비교/diff: 별도 phase.
- 배경 자동 재실행 (cron / interval): 사용자가 명령 트리거. 자동 스케줄러는 별도 phase.
- 부분 실패 자동 재시도 횟수 제한 / exponential backoff: 본 phase 는 `--retry-failed` 명시 트리거 시 한 번 더 시도.
- 세션 그룹별 다른 모델 사용: 단일 모델만.

## Success criteria

- `secall graph rebuild --since 2026-04-01` 실행 시 해당 기간 새 세션 + 미처리 세션 처리.
- `POST /api/commands/graph-rebuild` 트리거 → JobState 진행률 → 완료 시 `GraphRebuildOutcome` 반환.
- web UI Commands 페이지에서 "graph rebuild" 클릭 → 옵션 입력 → JobBanner 진행률 + 취소 버튼.
- DB v8 마이그레이션이 v7 DB 에서 멱등하게 적용되고 기존 데이터 손실 없음.
- 통합 테스트 1건 (Task 02): `--retry-failed` 가 `semantic_extracted_at IS NULL` 인 세션만 재처리하는지 검증.
