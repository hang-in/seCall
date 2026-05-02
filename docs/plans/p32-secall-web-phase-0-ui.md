---
type: plan
status: draft
updated_at: 2026-05-02
slug: p32-secall-web-phase-0-ui
version: 1
---

# P32 — secall-web Phase 0 (읽기 전용 웹 UI + 태그/즐겨찾기)

## Description

seCall에 셀프호스트 웹 GUI를 추가하는 첫 단계. 읽기 전용 브라우징(검색/세션/일기/위키/그래프)과 태그/즐겨찾기 편집까지 지원. 단일 바이너리에 정적 자산 임베드(release), 다크 모드 우선 모던 UI(2-pane + 그래프 폴딩 오버레이), Obsidian 플러그인과 동일 REST API 공유.

명령 트리거(sync/ingest/wiki update)와 SSE/Job 시스템은 Phase 1 (P33)으로 분리.

## Background

데스크탑 Opus와의 사전 토론으로 다음 결정 사항 확정:

| # | 결정 | 답 |
|---|---|---|
| A | 워크스페이스 디렉토리명 | `web/` (`obsidian-secall/`과 동일 레벨) |
| B | dev 모드 자산 서빙 | axum → Vite reverse proxy (단일 진입점) |
| C | CancellationToken | MVP 미포함, v1.1 |
| D | Job 큐 정책 | Read 무제한 동시, Write 단일 큐 (Phase 1에서 구현) |
| E | Job 영속성 | 진행 중=메모리, 완료=jobs 테이블, 시작 시 1회 cleanup (Phase 1) |
| F | 프론트 스택 | React Router v7 + Zustand + TanStack Query + RHF/Zod + Vite |
| G | API 버저닝 | 도입 안 함 (Obsidian과 단일 API 공유) |
| H | 배포 | GitHub Releases 바이너리 공식 채널, README에 cargo/brew 안내 |
| I | 클라이언트 분담 | Web=관리(Phase 1에서 명령 트리거), Obsidian=참조 |

## Expected Outcome

- `secall serve --port 8080` 실행 후 `http://127.0.0.1:8080` 접속 시 웹 UI 로드
- 검색 / 세션 상세 / 일일 일기 / 위키 / 그래프 탐색을 2-pane 레이아웃에서 사용
- 그래프 토글 버튼으로 오버레이 펼침/접힘, 노드 클릭 시 자동 폴딩 + 세션 로드
- 세션에 태그 추가/삭제, 즐겨찾기 토글 가능
- 다크 모드 + Pretendard(한글) + Geist Sans(영문) + shadcn/ui
- 단일 바이너리에 웹 자산 임베드 (release 빌드)
- Dev 모드: `cargo run -- serve` + `pnpm dev` 두 프로세스, axum이 5173으로 reverse proxy하여 단일 8080 진입점
- Obsidian 플러그인 호환성 유지 (기존 6개 엔드포인트 시그니처 변경 없음)

## Subtask Summary

| # | Title | Depends on | Parallel group |
|---|---|---|---|
| 00 | 워크스페이스 + 빌드 파이프라인 | — | A |
| 01 | rust-embed + Vite reverse proxy 통합 | 00 | B |
| 02 | 신규 REST 엔드포인트 추가 | 03 | C |
| 03 | DB 스키마 v5 마이그레이션 | 00 | B |
| 04 | React 프론트 핵심 셋업 | 00 | B |
| 05 | 2-pane 레이아웃 + 검색/세션 뷰 | 02, 04 | D |
| 06 | 일일 일기 + 위키 + 태그/즐겨찾기 UI | 02, 05 | E |
| 07 | 그래프 폴딩 오버레이 | 05 | E |
| 08 | README + CI 업데이트 | 01 | F |

## Constraints

- 기존 6개 엔드포인트 (`/api/recall`, `/api/get`, `/api/status`, `/api/wiki`, `/api/graph`, `/api/daily`) 시그니처 유지 — Obsidian 플러그인 호환
- API 버저닝 도입 안 함 (단일 `/api/*` 네임스페이스)
- 인증 미도입 — loopback 전용 유지 (`127.0.0.1` 바인딩, `crates/secall-core/src/mcp/rest.rs:124`)
- 세션 데이터(`sessions`, `turns`)는 읽기 전용 — 직접 편집 차단

## Non-goals

- WebSocket / SSE / Job 시스템 (Phase 1 = P33)
- 명령 트리거 (sync/ingest/wiki update) (Phase 1)
- 사용자 계정 / 멀티테넌시
- 모바일 반응형 (데스크탑 우선)
- Cancellation token (Phase 1 또는 v1.1)
- 워크플로우 트리거 / 알림 / 이메일
- AI 산출물(위키/세션) 직접 편집

## References

- 기존 REST 라우터: `crates/secall-core/src/mcp/rest.rs:93-110`
- 기존 서버 메서드: `crates/secall-core/src/mcp/server.rs` (`do_recall`, `do_get`, `do_status`, `do_wiki_search`, `do_graph_query`, `do_daily`)
- 기존 SessionRepo: `crates/secall-core/src/store/session_repo.rs`
- DB 마이그레이션 패턴: `crates/secall-core/src/store/db.rs:60-95`
- serve 명령: `crates/secall/src/commands/serve.rs`
- Obsidian 플러그인: `obsidian-secall/src/`
- CI 워크플로우: `.github/workflows/ci.yml`
- Release 워크플로우: `.github/workflows/release.yml`
