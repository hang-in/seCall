---
type: plan
status: draft
updated_at: 2026-05-02
slug: p34-secall-web-phase-2-ux
version: 1
---

# P34 — secall-web 뷰어 본격 강화 (Phase 2: 탐색 깊이 + UX)

## Description

P32 Phase 0(읽기 전용)과 P33 Phase 1(명령 트리거)이 끝난 상태에서, 사용자가 직접 "뷰어로써 부족하다"고 지적한 부분을 보강. 검색 깊이, 탐색 효율, 정보 밀도, 그래프 시각화, 사용자 메모를 한 단계 끌어올린다.

A (codesplit / 무한스크롤 / `/api/tags`)는 P35로 분리. B (Cancellation), C (시맨틱 graph sync)는 각각 P36/P37.

## Background — 결정 이력

| # | 결정 | 답 |
|---|---|---|
| 진행 순서 | P34=E → P35=A → P36=B → P37=C |
| 다국어 | 한국어/영문 README만 (UI 다국어 X — Phase 3+) |
| 다크/라이트 토글 | Phase 3+ — 본 plan은 다크 고정 유지 |
| 시맨틱 검색 graceful | Ollama 미설치 시 mode 토글 비활성 안내만 |
| 노트 편집 정책 | autosave 1초 debounce, 갈등 해결 last-write-wins |
| 키보드 단축키 라이브러리 | `react-hotkeys-hook` |
| 그래프 레이아웃 | dagre (`@dagrejs/dagre`) — elk보다 가볍고 충분 |
| 차트 | recharts vs SVG — recharts (~50KB, P35 codesplit 같이 진행) |

## Expected Outcome

- **검색 깊이**: 시맨틱 모드 동작, 검색어 하이라이트(SessionListItem + SessionDetail), 다중 태그 AND 필터, 날짜 quick range (오늘/이번주/이번달)
- **탐색 효율**: 키보드 단축키 — `j/k` 리스트 이동, `/` 검색 포커스, `?` 도움말, `g d/g w/g s` 라우트, `[/]` 세션 prev/next, `f` favorite toggle, `e` notes 편집
- **세션 컨텍스트**: 관련 세션 패널 (그래프 인접 + 같은 프로젝트/태그 N개)
- **그래프 시각화**: dagre 자동 레이아웃 + 노드 타입별 색상/아이콘 + 엣지 라벨 토글 + MiniMap 색상 매핑
- **정보 밀도**: 세션 헤더에 turn role 분포 + tool 사용 빈도 mini-chart
- **사용자 메모**: `sessions.notes TEXT` 컬럼 + `PATCH /api/sessions/{id}/notes` + autosave 노트 편집기

## Subtask Summary

| # | Title | Depends on | Parallel group |
|---|---|---|---|
| 00 | DB 스키마 v7 (`notes` 컬럼) + REST PATCH | — | A |
| 01 | 시맨틱 검색 모드 활성 (`/api/recall` 분기 + Ollama graceful) | — | A |
| 02 | 검색어 하이라이트 (SessionListItem + MarkdownView) | — | A |
| 03 | 다중 태그 필터 + 날짜 quick range | — | A |
| 04 | 키보드 단축키 + `?` 도움말 다이얼로그 | — | A |
| 05 | 관련 세션 패널 (SessionDetail 하단) | 00 | B |
| 06 | 그래프 시각화 강화 (dagre + 노드 색상/아이콘) | — | A |
| 07 | 세션 메타 mini-chart (role 분포 + tool 빈도) | — | A |
| 08 | 세션 노트 편집 UI (autosave 1s debounce) | 00 | B |
| 09 | README + CI | 00, 01, 04 | C |

## Constraints

- 기존 19개 엔드포인트 시그니처 유지 (Obsidian 호환)
- API 버저닝 도입 안 함
- 인증 미도입 (loopback 전용)
- 시맨틱 검색은 Ollama 미설치 시 graceful degradation
- 다크 모드 고정 (라이트 모드 Phase 3+)

## Non-goals

- 무한 스크롤 / codesplit / 전용 `/api/tags` 엔드포인트 — P35 (A)
- Cancellation token (현재 501) — P36 (B)
- 시맨틱 graph sync 통합 — P37 (C)
- 다크/라이트 모드 토글 — Phase 3+
- 다국어 (UI 한국어 외) — Phase 3+
- 세션 비교 (side-by-side) — Phase 3+
- 세션 익스포트 (markdown download) — Phase 3+

## References

- P32 메인 플랜: `docs/plans/p32-secall-web-phase-0-ui.md`
- P33 메인 플랜: `docs/plans/p33-secall-web-phase-1-sse-job.md`
- DB 마이그레이션 패턴: `crates/secall-core/src/store/db.rs:60-100`
- REST 라우터: `crates/secall-core/src/mcp/rest.rs`
- 그래프 데이터: `crates/secall-core/src/graph/`
- 기존 Web UI: `web/src/{routes,components,hooks,lib}/`
