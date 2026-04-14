---
type: plan
status: in_progress
updated_at: 2026-04-14
slug: p25-semantic-graph-obsidian-phase-0-1
---

# P25 — Semantic Graph 활용 + Obsidian 플러그인 (Phase 0-1)

## Background

694세션에서 추출한 knowledge graph (3,433 노드, 9,152 엣지)의 활용도를 높이고,
seCall을 Obsidian 안에서 직접 사용할 수 있게 한다.

Phase 0-1 범위: REST API 레이어 + Obsidian 플러그인 MVP.
Phase 2-3 (데일리 노트, Canvas 시각화 등)은 완료 후 별도 플랜으로 진행.

## 아키텍처

```
┌─ Obsidian Plugin (TypeScript) ─┐
│  requestUrl()  → 단건 요청      │
│  settings: 서버 주소 설정       │
└────────────┬───────────────────┘
             │ HTTP (localhost)
             ▼
┌─ secall serve --port 8080 ─────────────────┐
│  axum Router                                │
│  ├─ POST /api/recall   ──┐                 │
│  ├─ POST /api/get      ──┤                 │
│  ├─ GET  /api/status   ──┤→ do_*() 공통    │
│  ├─ POST /api/wiki     ──┤   로직 메서드   │
│  ├─ POST /api/graph    ──┘                 │
│  └─ /mcp               ← 기존 MCP (Claude Code용) │
└────────────────────────────────────────────┘
```

핵심: `SeCallMcpServer`에 `do_*()` pub 메서드를 추출하여 공통 레이어로 사용.
REST와 MCP 모두 이 메서드를 호출하되, 각각의 응답 형식으로 래핑.

## Subtasks

| # | Phase | 제목 | 핵심 파일 | depends_on |
|---|-------|------|-----------|------------|
| 01 | 0 | REST API 서버 (`secall serve`) | `mcp/server.rs`, `mcp/rest.rs`(신규), `commands/serve.rs`(신규) | 없음 |
| 02 | 1 | Obsidian 플러그인 scaffold + recall | `obsidian-secall/`(신규 디렉토리) | 01 |
| 03 | 1 | 세션 조회 + 상태바 | `obsidian-secall/src/` | 02 |

Task 02~03: 순차 (01 완료 후).

## 활용 시나리오

### Phase 0 완료 시
- `secall serve --port 8080` 실행
- curl/브라우저에서 REST API 직접 호출 가능
- 외부 스크립트(Python 등)에서 seCall 데이터 활용 가능

### Phase 1 완료 시 (MVP)
- Obsidian에서 Cmd+P → "seCall: Search" → 검색어 입력
- 사이드바에 검색 결과 목록 표시
- 결과 클릭 → vault 내 세션 MD 파일 바로 열기
- 하단 상태바에 "seCall: 694 sessions, vectors ✓" 표시

## 비용/위험

| 위험 | 대응 |
|------|------|
| REST + MCP 이중 API | `do_*()` 공통 레이어로 로직 중복 제거 |
| Obsidian 모바일 | requestUrl 모바일 지원. child_process 미사용 |
| 플러그인 배포 | 초기에는 수동 설치 (BRAT), 안정화 후 커뮤니티 스토어 |
| `#[tool_router]` 매크로 간섭 | `do_*`는 `#[tool]` 없는 일반 메서드 — 매크로 영향 없음 |

## 향후 작업 (Phase 2-3, 이 플랜 범위 밖)

- Phase 2: 데일리 노트 자동 생성, Graph 탐색 뷰, vault wikilink 자동 생성
- Phase 3: Ingest 트리거 + SSE 진행률, Canvas 그래프 시각화

## 테스트 기준선 (2026-04-14)

```
secall-core: 253 passed, 0 failed, 10 ignored
secall:      16 passed (+ 4 integration)
총:          273 passed
```
