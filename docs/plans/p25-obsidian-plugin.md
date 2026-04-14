---
type: plan
status: in_progress
updated_at: 2026-04-14
slug: p25-obsidian-plugin
---

# P25 — Semantic Graph 활용 + Obsidian 플러그인

## Background

694세션에서 추출한 knowledge graph (3,433 노드, 9,152 엣지)의 활용도를 높이고,
seCall을 Obsidian 안에서 직접 사용할 수 있게 한다.

## 아키텍처

```
┌─ Obsidian Plugin (TypeScript) ─┐
│  requestUrl()  → 단건 요청      │
│  EventSource() → 실시간 스트림   │
└────────────┬───────────────────┘
             │ HTTP (localhost)
             ▼
┌─ secall serve ─────────────────────────┐
│  axum Router                            │
│  ├─ POST /api/recall   ──┐             │
│  ├─ POST /api/get      ──┤             │
│  ├─ GET  /api/status   ──┼→ 공통 로직   │
│  ├─ POST /api/wiki     ──┤  (기존 MCP  │
│  ├─ POST /api/graph    ──┘   tool 함수) │
│  └─ GET  /api/events   ← SSE 스트림    │
├─────────────────────────────────────────┤
│  /mcp  ← 기존 MCP (Claude Code용)      │
└─────────────────────────────────────────┘
```

핵심: MCP tool 로직을 공통 레이어로 두고, REST와 MCP가 각각 진입점 역할만 수행.

## Phases

### Phase 0 — seCall REST API 레이어 (Rust)

**목표**: `secall serve --port 8080`으로 REST + SSE 엔드포인트 노출

**엔드포인트**:
| Method | Path | MCP Tool | 설명 |
|--------|------|----------|------|
| POST | /api/recall | recall | 세션 검색 (keyword/semantic/temporal) |
| POST | /api/get | get | 세션/turn 조회 |
| GET | /api/status | status | 인덱스 상태 |
| POST | /api/wiki | wiki_search | wiki 페이지 검색 |
| POST | /api/graph | graph_query | 그래프 노드 탐색 |
| POST | /api/ingest | (신규) | ingest 트리거 |
| GET | /api/events | (신규) | SSE 이벤트 스트림 |

**구현**:
- `crates/secall/src/commands/serve.rs` 신규 생성
- 기존 `SeCallMcpServer`의 메서드를 직접 호출
- loopback 제한 유지 (보안)
- CORS 헤더 추가 (Obsidian app:// origin 허용)

**공수**: 2~3일

### Phase 1 — Obsidian 플러그인 MVP

**목표**: Obsidian 안에서 seCall 검색 + 세션 조회

**기능**:
1. 설정 패널: 서버 주소 (기본 127.0.0.1:8080)
2. 커맨드 팔레트: `seCall: Search` → 검색어 입력 모달
3. 검색 결과 ItemView: 사이드바에 결과 목록, 클릭 시 세션 MD 파일 열기
4. 상태바: 세션 수, 벡터 상태 표시
5. 리본 아이콘: 검색 패널 토글

**기술 스택**:
- obsidian-sample-plugin 템플릿
- esbuild 번들링
- requestUrl()로 REST API 호출 (MCP SDK 불필요)

**공수**: 1~2주

### Phase 2 — 데일리 노트 + Graph 탐색

**목표**: 매일의 작업을 자동 정리 + 그래프 기반 탐색

**기능**:
1. 데일리 노트 자동 생성
   - `POST /api/recall` (temporal: "today") → 오늘 세션 목록
   - `POST /api/get` (full: true) → 각 세션 요약
   - obsidian-daily-notes-interface로 노트 생성
   - 템플릿: 프로젝트별 그룹핑, 이슈/결정사항 하이라이트
2. Graph 탐색 뷰
   - `POST /api/graph` → 노드 이웃 조회
   - 인터랙티브 트리 뷰 (노드 클릭 → 확장)
3. vault 세션 MD에 wikilink 생성
   - `secall graph link` CLI 명령
   - 세션 하단에 Related 섹션 자동 추가
   - Obsidian Graph View에서 시각적 탐색

**공수**: 1~2주

### Phase 3 — 고급 기능

**목표**: 동기화, 실시간 피드백, Canvas 시각화

**기능**:
1. Ingest 트리거 + SSE 진행률
   - 플러그인에서 ingest 버튼 → `POST /api/ingest`
   - `GET /api/events` SSE로 진행률 수신
2. Sync 상태 모니터링
3. Canvas 그래프 시각화
   - graph_query 결과를 .canvas JSON으로 변환
   - Obsidian Canvas에서 네이티브 시각화

**공수**: 2~3주

## Subtasks (P25 범위: Phase 0-1)

| # | Phase | 제목 | 핵심 파일 | depends_on |
|---|-------|------|-----------|------------|
| 01 | 0 | REST API 서버 (`secall serve`) | commands/serve.rs (신규) | 없음 |
| 02 | 1 | Obsidian 플러그인 scaffold + recall | obsidian-secall/ (신규 디렉토리) | 01 |
| 03 | 1 | 세션 조회 + 상태바 | obsidian-secall/src/ | 02 |

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
| REST + MCP 이중 API | 공통 로직 레이어로 해결. REST는 얇은 라우터만 |
| Obsidian 모바일 | requestUrl 모바일 지원. child_process는 데스크톱 전용이므로 서버 자동 시작만 제한 |
| 플러그인 배포 | 초기에는 수동 설치 (BRAT), 안정화 후 커뮤니티 스토어 등록 |

## 향후 작업 (Phase 2-3, P25 범위 밖)

### Phase 2 — 데일리 노트 + Graph 탐색
- 데일리 노트 자동 생성 (temporal recall → 프로젝트별 그룹핑)
- Graph 탐색 뷰 (인터랙티브 트리)
- vault 세션 MD에 wikilink 자동 생성 (`secall graph link`)

### Phase 3 — 고급 기능
- Ingest 트리거 + SSE 진행률
- Sync 상태 모니터링
- Canvas 그래프 시각화

## 테스트 기준선 (2026-04-14)

```
secall-core: 253 passed, 0 failed, 10 ignored
secall:      16 passed (+ 4 integration)
총:          273 passed
```
