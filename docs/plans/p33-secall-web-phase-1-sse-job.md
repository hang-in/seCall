---
type: plan
status: draft
updated_at: 2026-05-02
slug: p33-secall-web-phase-1-sse-job
version: 1
---

# P33 — secall-web Phase 1 (명령 트리거 + SSE + Job 시스템)

## Description

P32 Phase 0의 후속 단계. 웹 UI에서 sync / ingest / wiki update 같은 장기 실행 명령을 트리거하고, SSE로 진행 상태를 스트리밍한다. mutating 작업은 단일 큐로 직렬화하고, 진행 중 상태는 메모리에, 완료 상태는 `jobs` 테이블에 기록한다.

P32에서 미루었던 Wiki 본문 fetch 엔드포인트도 본 plan에 함께 포함하며, 그래프 자동 증분(ingest 후 graph_nodes/graph_edges 갱신) 옵션도 추가한다.

## Background — P32에서 확정된 결정

| # | 결정 | 답 |
|---|---|---|
| Job 큐 정책 | Read 무제한 동시, Write(sync/ingest/wiki update)는 단일 큐 |
| 동시 실행 거부 | 같은 종류 중복 → 409 Conflict, 다른 종류 충돌 → 거부 ("이미 실행 중") |
| Job 영속성 | 진행 중 = 메모리 (Arc<RwLock<HashMap>>), 완료 = `jobs` 테이블 |
| Cleanup | 서버 시작 시 1회, 7일 이상된 jobs row DELETE |
| 진행 상태 전송 | SSE (axum 네이티브) — WebSocket 미사용 |
| Cancellation | MVP 미포함 (phase 경계 단위만 — v1.1) |
| 부분 성공 | sync의 phase별 결과(pull/reindex/ingest/push) 명확히 분리 |
| API 버저닝 | 도입 안 함 (`/api/*` 단일 네임스페이스) |
| 인증 | loopback 전용 유지 |

## Expected Outcome

- 웹 UI 사이드바에 "Commands" 메뉴 또는 우상단 액션 — `Sync`, `Ingest`, `Wiki Update` 버튼
- 버튼 클릭 시 `POST /api/commands/<name>` → `{ job_id, status: "started" }` 즉시 응답
- 화면 상단 글로벌 배너에 "현재 작업 N개 실행 중" + 진행률 phase 표시
- SSE 스트림으로 phase별 실시간 진행 상태 (`pull → reindex → ingest → push`)
- 탭 닫고 재접속 시 진행 중 작업 자동 감지 + SSE 재연결
- 작업 완료/실패 시 toast 알림 + 결과 (예: "N개 신규 세션 ingest")
- 부분 성공 명시 (예: "ingest까지 OK / push 실패: <error>")
- WikiRoute가 preview 대신 전체 본문 표시 (P32 잔여)
- `secall ingest` / `sync`에 `--auto-graph` 옵션 — 신규 세션의 그래프 자동 증분

## Subtask Summary

| # | Title | Depends on | Parallel group |
|---|---|---|---|
| 00 | DB 스키마 v6 — `jobs` 테이블 + cleanup | — | A |
| 01 | `Job` 코어 모듈 (registry / executor / 단일 큐) | 00 | B |
| 02 | Job → 명령 어댑터 (sync / ingest / wiki update) | 01 | C |
| 03 | REST 엔드포인트 — Jobs (POST commands, GET jobs, SSE stream) | 02 | D |
| 04 | Wiki 본문 fetch 엔드포인트 + UI | — | A |
| 05 | Web UI — Commands 패널 + Job 시작/상태 훅 | 03, 04 | E |
| 06 | Web UI — 글로벌 진행 배너 + SSE 재연결 | 05 | F |
| 07 | ingest 후 graph 자동 증분 (옵션 플래그) | 02 | C |
| 08 | README + CI 업데이트 | 03, 05, 06 | G |

## Constraints

- 기존 6개 + P32의 5개 = 11개 엔드포인트 시그니처 유지 (Obsidian 호환)
- API 버저닝 도입 안 함
- 인증 미도입 (loopback 전용)
- Cancellation은 phase 경계 단위만 (MVP)
- `jobs` 테이블 cleanup은 시작 시 1회만 (백그라운드 주기 작업은 v1.1)

## Non-goals

- 사용자 계정 / 멀티테넌시 / 권한
- WebSocket (SSE만)
- 작업 우선순위 / Job 종속성 그래프
- 진행 중 LLM 호출 abort
- 모바일 반응형 UI

## References

- 기존 sync (4-phase 파이프라인): `crates/secall/src/commands/sync.rs:14`
- 기존 ingest: `crates/secall/src/commands/ingest.rs:49` (CLI), `:147` (`ingest_sessions`)
- 기존 wiki: `crates/secall/src/commands/wiki.rs:9` (`run_update`)
- 기존 graph build/semantic: `crates/secall/src/commands/graph.rs:131` (`run_build`), `:10` (`run_semantic`)
- REST 라우터: `crates/secall-core/src/mcp/rest.rs`
- DB 마이그레이션 패턴: `crates/secall-core/src/store/db.rs:60-95`
- P32 Web UI: `web/src/{routes,components,hooks,lib}/`
