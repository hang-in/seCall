<!-- Thanks to: @batmania52, @yeonsh, @missflash, @CoLuthien, @dev-minsoo -->

<div align="center">

# seCall

AI 에이전트와 나눈 모든 대화를 검색하세요.

**Search everything you've ever discussed with AI agents.**

[![Rust](https://img.shields.io/badge/Rust-1.75+-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5-003B57?logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![MCP](https://img.shields.io/badge/MCP-Protocol-5A67D8?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0Ij48Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIgZmlsbD0id2hpdGUiLz48L3N2Zz4=)](https://modelcontextprotocol.io/)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![ONNX Runtime](https://img.shields.io/badge/ONNX-Runtime-007CFF?logo=onnx&logoColor=white)](https://onnxruntime.ai/)
[![Obsidian](https://img.shields.io/badge/Obsidian-Plugin-7C3AED?logo=obsidian&logoColor=white)](https://obsidian.md/)

<br/>

**`한국어`** · [**`English`**](README.en.md) · [**`日本語`**](README.ja.md) · [**`中文`**](README.zh.md)

</div>

---

## 목차

- [seCall이란?](#secall이란)
- [주요 기능](#주요-기능)
  - [멀티 에이전트 수집](#멀티-에이전트-수집)
  - [하이브리드 검색](#하이브리드-검색)
  - [지식 볼트](#지식-볼트)
  - [Knowledge Graph](#knowledge-graph)
  - [REST API + Obsidian 플러그인](#rest-api--obsidian-플러그인)
  - [MCP 서버](#mcp-서버)
  - [멀티 기기 볼트 동기화](#멀티-기기-볼트-동기화)
  - [데이터 무결성](#데이터-무결성)
- [빠른 시작](#빠른-시작)
  - [사전 요구사항](#사전-요구사항)
  - [Step 1. 설치](#step-1-설치)
  - [Step 2. 초기화](#step-2-초기화)
  - [Step 3. 세션 수집](#step-3-세션-수집)
  - [Step 4. 검색](#step-4-검색)
- [사용법](#사용법)
  - [세션 조회](#세션-조회)
  - [임베딩 생성](#임베딩-생성)
  - [세션 분류](#세션-분류)
  - [위키 생성](#위키-생성)
  - [작업 일기](#작업-일기)
  - [Knowledge Graph](#knowledge-graph-1)
- [설정](#설정)
  - [설정 키 목록](#설정-키-목록)
- [CLI 레퍼런스](#cli-레퍼런스)
- [MCP 연동](#mcp-연동)
- [아키텍처](#아키텍처)
- [기술 스택](#기술-스택)
- [출처](#출처)
- [라이선스](#라이선스)

---

<div align="center">
<img src="screenshot.png" alt="seCall Obsidian 볼트" width="720" />
<br/><br/>
</div>

## seCall이란?

seCall은 AI 에이전트 세션을 위한 로컬 퍼스트 검색 엔진입니다. **Claude Code**, **Codex CLI**, **Gemini CLI**, **claude.ai**, **ChatGPT**의 대화 로그를 수집하고, BM25 + 벡터 하이브리드 검색으로 인덱싱하며, CLI/MCP 서버/Obsidian 호환 지식 볼트로 제공합니다.

AI와의 대화는 곧 지식 자산입니다. seCall은 그것을 검색 가능하고, 탐색 가능하며, 서로 연결된 형태로 만듭니다.

### 왜 필요한가?

- 수백 개의 에이전트 세션에 걸쳐 아키텍처, 디버깅, 설계 결정을 논의했지만 — 불투명한 JSONL 파일에 흩어져 있습니다.
- seCall은 이 세션들을 **구조화되고 검색 가능한 지식 그래프**로 변환합니다. MCP 호환 AI 에이전트에서 쿼리하거나 Obsidian에서 탐색할 수 있습니다.

## 주요 기능

### 멀티 에이전트 수집

여러 AI 코딩 에이전트의 세션을 통합 형식으로 파싱하고 정규화합니다:

| 에이전트 | 형식 | 상태 |
|---|---|---|
| Claude Code | JSONL | ✅ 안정 |
| Codex CLI | JSONL | ✅ 안정 |
| Gemini CLI | JSON | ✅ 안정 |
| claude.ai | JSON (ZIP) | ✅ v0.2 신규 |
| ChatGPT | JSON (ZIP) | ✅ v0.2.3 신규 |

### 하이브리드 검색

- **BM25 전문 검색**: SQLite FTS5 + 한국어 형태소 분석 ([Lindera](https://github.com/lindera/lindera) ko-dic / [Kiwi-rs](https://github.com/bab2min/kiwi) 선택 가능)
- **벡터 시맨틱 검색**: [Ollama](https://ollama.com/) BGE-M3 임베딩 (1024차원) + **HNSW ANN 인덱스** ([usearch](https://github.com/unum-cloud/usearch))로 O(log n) 탐색
- **Reciprocal Rank Fusion (RRF)**: BM25/벡터 독립 실행 후 결합 (k=60) + **세션 다양성 강제** (세션당 최대 2개 턴)
- **LLM 쿼리 확장**: Claude Code를 통한 자연어 쿼리 확장

### 지식 볼트

Obsidian 호환 마크다운 볼트 (2계층 구조):

```
vault/
├── raw/sessions/    # 불변 세션 원본
│   └── YYYY-MM-DD/  # 날짜별 정리
├── wiki/            # AI 생성 지식 페이지
│   ├── projects/    # 프로젝트별 요약
│   ├── topics/      # 기술 주제 페이지
│   └── decisions/   # 아키텍처 의사결정 기록
└── graph/           # Knowledge Graph 출력
    └── graph.json   # 노드/엣지 데이터
```

- **위키 생성**: pluggable LLM backend 기반 (`secall wiki update --backend claude|codex|ollama|lmstudio`)
- **Obsidian 백링크** (`[[]]`)로 세션 ↔ 위키 페이지 연결
- Dataview 쿼리를 위한 frontmatter 메타데이터 (`summary` 필드로 세션 내용 즉시 파악)

### Knowledge Graph

세션 간 관계를 추출하여 지식 그래프를 구축합니다:

- **노드 타입**: session, project, agent, tool — frontmatter에서 자동 추출
- **규칙 기반 엣지**: `belongs_to`, `by_agent`, `uses_tool`, `same_project`, `same_day` (LLM 불필요)
- **시맨틱 엣지** (Gemini/Ollama/LM Studio): `fixes_bug`, `modifies_file`, `introduces_tech`, `discusses_topic` — LLM이 세션 내용을 분석하여 추출
- **증분 빌드**: 신규 세션만 노드 추가, 관계 엣지는 전체 재계산으로 정확성 보장
- **MCP 도구**: `graph_query` — AI 에이전트가 세션 간 관계를 탐색 (BFS, 최대 3홉)

### Web UI + REST API + Obsidian 플러그인

`secall serve`는 REST API와 웹 UI를 동일 포트(8080)에서 제공하며, Obsidian 플러그인과도 동일 API를 공유합니다.

```bash
# REST API + Web UI 서버 시작
secall serve --port 8080
# 브라우저: http://127.0.0.1:8080
```

**엔드포인트**:
- 읽기 (Phase 0): `/api/recall`, `/api/get`, `/api/status`, `/api/daily`, `/api/graph`, `/api/wiki` (검색)
- 위키 본문 (Phase 1): `GET /api/wiki/{project}`
- 세션 메타 (Phase 0): `/api/sessions`, `/api/projects`, `/api/agents`, `PATCH /api/sessions/{id}/{tags,favorite}`
- 세션 노트 (Phase 2): `PATCH /api/sessions/{id}/notes`
- 태그 목록 (Phase 3): `GET /api/tags?with_counts={true|false}`
  - `true` (기본): `{ "tags": [{ "name": "rust", "count": 12 }, ...] }`
  - `false`: `{ "tags": ["rust", "search", ...] }`
- 명령 (Phase 1): `POST /api/commands/{sync,ingest,wiki-update}`
- Job 관리 (Phase 1): `GET /api/jobs`, `GET /api/jobs/{id}`, `GET /api/jobs/{id}/stream` (SSE)
- Job 취소 (P36): `POST /api/jobs/{id}/cancel`
  - 200: `{ "cancelled": true, "job_id": "..." }` — 활성 job 취소 성공 (이미 완료/취소된 job 도 동일 응답으로 idempotent)
  - 404: `{ "error": "job not found or already evicted" }` — 미등록 / evict 됨

**Web UI** (`web/`, P32 Phase 0 + P33 Phase 1):
- 다크 모드 우선 모던 UI (Tailwind + shadcn/ui + Pretendard/Geist Sans)
- 2-pane 레이아웃 (좌: 검색/리스트, 우: 상세)
- 그래프 폴딩 오버레이 (노드 클릭 → 세션 로드 + 자동 폴딩)
- 태그 / 즐겨찾기 편집
- 사이드바 **Commands** 메뉴 — Sync / Ingest / Wiki Update 트리거 (Phase 1)
- 글로벌 진행 배너 + SSE 진행 스트리밍 + 완료/실패 toast (Phase 1)

**Obsidian 플러그인** (`obsidian-secall/`):
- **검색 뷰** — 키워드/시맨틱 세션 검색
- **데일리 뷰** — 날짜별 작업 요약, 프로젝트별 세션 그룹핑, 노트 생성
- **그래프 뷰** — 노드 관계 탐색 (depth 1-3, 관계 필터)
- **세션 뷰** — 전체 마크다운 렌더링
- **상태바** — 세션 수 + 임베딩 상태 표시 (5분 갱신)

### MCP 서버

MCP 호환 AI 에이전트에 세션 인덱스를 노출합니다:

```bash
# stdio 모드 (Claude Code, Cursor 등)
secall mcp

# HTTP 모드 (웹 클라이언트)
secall mcp --http 127.0.0.1:8080
```

제공 도구: `recall`, `get`, `status`, `wiki_search`, `graph_query`

### 멀티 기기 볼트 동기화

Git을 통해 여러 기기에서 지식 볼트를 동기화합니다:

```bash
# 전체 동기화: git pull → reindex → ingest → wiki → graph → git push
secall sync

# 로컬 전용 모드 (git 생략, Claude Code hook에 적합)
secall sync --local-only
```

- **MD가 원본** — DB는 파생 캐시이며, `secall reindex --from-vault`로 완전 복구 가능
- **호스트 추적** — 각 세션이 어떤 기기에서 수집되었는지 기록 (frontmatter `host` 필드)
- **충돌 없음** — 세션은 기기별 유니크하므로 git 머지 충돌 없음

### 데이터 무결성

내장 린트 규칙으로 인덱스 ↔ 볼트 정합성을 검증합니다:

```bash
secall lint
# L001: 누락된 볼트 파일
# L002: 고아 볼트 파일
# L003: FTS 인덱스 갭
```

## 빠른 시작

### 사전 요구사항

- Rust 1.75+ (소스 빌드 시)
- Claude Code, Codex CLI, Gemini CLI 중 하나 이상
- [Ollama](https://ollama.com/) — 벡터 검색용 (선택사항, 없으면 BM25만 사용)
- **Windows**: MSVC 툴체인 (Visual Studio Build Tools)

### Step 1. 설치

**GitHub Releases (권장)** — 웹 UI 포함된 단일 바이너리:

[Releases 페이지](https://github.com/hang-in/seCall/releases)에서 OS에 맞는 파일 다운로드.
- macOS: `secall-aarch64-apple-darwin.tar.gz` / `secall-x86_64-apple-darwin.tar.gz`
- Windows: `secall-x86_64-pc-windows-msvc.zip` (secall.exe + onnxruntime.dll)

**Cargo (개발자용)**:

```bash
# CLI/MCP/REST API만 (웹 UI 미포함)
cargo install --path crates/secall --no-default-features

# 웹 UI 포함 — Node 22 + pnpm 9 + just 사전 설치 필요
git clone https://github.com/hang-in/seCall.git && cd seCall
just build         # web/dist 빌드 → cargo build --release
cp target/release/secall ~/.local/bin/
```

> `cargo install secall`은 npm 빌드를 자동으로 수행하지 않습니다. 웹 UI를 사용하려면 Releases 바이너리 또는 위의 직접 빌드를 사용하세요.

**Homebrew** (예정 — tap 등록 작업 진행 중):

```bash
brew install hang-in/tap/secall
```

> **Windows 사용자**: 핵심 기능(파싱, BM25 검색, vault, MCP)은 동일하게 동작합니다. 아래 기능은 MSVC 미지원으로 비활성화:
> - **HNSW ANN 인덱스** (`usearch`) — BLOB 코사인 스캔 fallback
> - **Kiwi-rs 형태소 분석** — Lindera ko-dic fallback

### Step 2. 초기화

```bash
# 대화형 온보딩 (권장)
secall init

# 또는 인자 직접 지정
secall init --vault ~/Documents/Obsidian\ Vault/seCall
secall init --git git@github.com:you/obsidian-vault.git
```

`secall init`을 인자 없이 실행하면 대화형 위저드가 시작됩니다:
- Vault 경로 설정
- Git remote (선택)
- 토크나이저 선택 (lindera/kiwi)
- 임베딩 백엔드 선택 (ollama/none)
- Ollama 설치 확인 + `bge-m3` 모델 자동 pull

### Step 3. 세션 수집

```bash
# Claude Code 세션 자동 감지
secall ingest --auto

# Codex CLI / Gemini CLI
secall ingest ~/.codex/sessions
secall ingest ~/.gemini/sessions

# claude.ai / ChatGPT export (ZIP)
secall ingest ~/Downloads/data-export.zip

# 또는 한 명령으로 전체 동기화
secall sync
```

### Step 4. 검색

```bash
# BM25 전문 검색
secall recall "BM25 인덱싱 구현"

# 프로젝트, 에이전트, 날짜 필터
secall recall "에러 처리" --project seCall --agent claude-code --since 2026-04-01

# 벡터 시맨틱 검색 (Ollama 필요)
secall recall "검색 파이프라인 동작 방식" --vec

# LLM 쿼리 확장
secall recall "검색 정확도 개선" --expand
```

## Web UI

`secall serve`는 REST API와 함께 웹 UI를 동일 포트에서 제공합니다 (단일 진입점).

```bash
secall serve --port 8080
# 브라우저에서 http://127.0.0.1:8080 접속
```

**Phase 0 기능** (P32, 읽기 전용):
- 검색 / 세션 브라우징 (2-pane 레이아웃)
- 일일 일기 / 위키 페이지 열람 (전체 본문 — Phase 1에서 위키 본문 fetch 추가)
- 그래프 탐색 (사이드바 Graph 버튼 → 풀스크린 오버레이)
- 태그 / 즐겨찾기 편집

**Phase 1 기능** (P33, 명령 트리거):
- 사이드바 **Commands** 메뉴 — Sync / Ingest / Wiki Update 버튼 + 옵션 다이얼로그
- SSE 진행 스트리밍 — phase별 실시간 표시
- 글로벌 진행 배너 — 어떤 페이지에서든 활성 작업 추적 (sticky top)
- 완료/실패/중단 자동 toast 알림
- 부분 성공 명시 (예: "ingest까지 OK / push 실패")
- 한 번에 하나의 mutating 작업만 실행 (단일 큐)
- 탭 닫고 재접속 시 진행 중 작업 자동 복원

**Phase 2 기능** (P34, 뷰어 강화):
- 시맨틱 검색 모드 토글 (Ollama 사용 시)
- 검색어 하이라이트 — 리스트 + 마크다운 본문 양쪽
- 다중 태그 AND 필터 + 날짜 quick range (오늘/이번 주/이번 달)
- 키보드 단축키 — `?` 도움말, `j/k` 리스트 이동, `/` 검색 포커스, `g d/w/s/c` 라우트, `[/]` 세션 prev/next, `f` 즐겨찾기, `e` 노트
- 관련 세션 패널 — 그래프 인접 + 같은 프로젝트/태그 추천 (세션 상세 하단)
- 그래프 시각화 강화 — dagre 자동 레이아웃 + 노드 타입별 색상/아이콘 + 엣지 라벨 토글 + 범례
- 세션 메타 mini-chart — turn role 분포 (user/assistant/system) + tool 사용 빈도 top 5
- 사용자 노트 편집 — 세션별 markdown 노트 (autosave 1s, `PATCH /api/sessions/{id}/notes`)

**Phase 3 기능** (P35, 성능 + 정확도):
- `/api/tags` 엔드포인트 — 모든 태그 + 사용 빈도 정확 노출 (sessions 100건 휴리스틱 제거)
- SessionList 무한 스크롤 — IntersectionObserver 기반 자동 로드 (page_size=100)
- Code-split — 라우트별 + vendor (react/query/radix/viz) chunk 분리, 초기 진입 JS ≤ 250 kB (gzip)

**Job Cancellation** (P36, 실행 중 작업 취소):
- 실행 중 sync / ingest / wiki-update 작업을 안전하게 중단 가능
- `tokio_util::sync::CancellationToken` 기반 — `JobRegistry` / `JobExecutor` / `BroadcastSink` 통합, `ProgressSink::is_cancelled()` 노출
- 어댑터(sync/ingest/wiki) 가 안전 지점에서 polling — phase 사이, file/session 루프 시작, LLM 호출 직전
- 부분 결과 보존 — 예: ingest 100건 중 50건 처리 후 취소 → 결과 JSON 에 `ingested=50` 그대로 기록
- 취소 시 최종 SSE 이벤트: `Failed { error: "cancelled by user", partial_result: None }`, job 상태는 `Interrupted` 로 강제
- REST: `POST /api/jobs/{id}/cancel` — 활성 200, idempotent 200, 미등록/evict 404
- Web UI: `JobBanner` 와 활성 `JobItem` 에 **취소** 버튼 + `window.confirm` 다이얼로그 (`useCancelJob` mutation hook)

### 키보드 단축키 (Phase 2)

| 키 | 동작 |
|---|---|
| `?` | 단축키 도움말 |
| `/` | 검색 포커스 |
| `j` / `k` | 리스트 다음/이전 항목 |
| `[` / `]` | 세션 prev/next |
| `g d` | Daily 화면 |
| `g w` | Wiki 화면 |
| `g s` | Sessions 화면 |
| `g c` | Commands 화면 |
| `g g` | 그래프 오버레이 토글 |
| `f` | 현재 세션 즐겨찾기 토글 |
| `e` | 현재 세션 노트 편집 |
| `Esc` | 다이얼로그/오버레이 닫기 |

### 명령 사용

웹 UI에서 좌측 사이드바 **Commands** 메뉴 → 원하는 명령 + 옵션 → 시작.

CLI에서도 동일하게 사용 가능 (Job 시스템은 웹 UI 전용):
```bash
secall sync --local-only --dry-run
secall sync --no-graph         # graph 자동 증분 비활성 (sync 기본은 활성)
secall ingest --auto --auto-graph   # ingest 시 graph 자동 증분 활성 (기본 비활성)
secall wiki update --backend claude
```

### Job 시스템

명령 트리거(sync/ingest/wiki update)는 백그라운드 Job으로 실행됩니다:

1. `POST /api/commands/{kind}` → 즉시 `{ job_id, status: "started" }` 응답 (HTTP 202)
2. 진행 중 상태는 메모리에 저장되어 빠른 SSE/폴링 가능 (`Arc<RwLock<HashMap>>`)
3. 완료/실패 시 `jobs` 테이블에 영구 기록
4. **단일 큐**: 동시에 mutating 작업은 1개만 — 두 번째 요청은 `409 Conflict` + `{"error":"another mutating job is running","current_kind":"sync|ingest|wiki_update"}`
5. **Read 작업** (검색, 세션 조회 등)은 동시 무제한
6. 서버 재시작 시 `running`/`started` 상태 jobs는 자동으로 `interrupted`로 갱신
7. 7일 이상된 완료/실패/중단 jobs는 시작 시 자동 cleanup
8. **Cancellation 지원** (P36) — `POST /api/jobs/{id}/cancel` 로 활성 job 취소 (200 idempotent / 404 unknown). 어댑터가 phase 사이·루프·LLM 호출 직전 안전 지점에서 polling 하여 부분 결과를 보존하고, job 상태는 `Interrupted` 로 종료

#### Phase 분리 (sync 예시)

```
sync = init → pull → reindex → ingest → wiki_update → graph → push
```

각 phase 완료마다 SSE 이벤트 발행 (`type` discriminator: `initial_state`, `phase_start`, `message`, `progress`, `phase_complete`, `done`, `failed`, KeepAlive 15초). push 실패 시 ingest까지의 결과는 보존되며 결과 JSON에 명시:

```json
{
  "pulled": 3,
  "reindexed": 5,
  "ingested": 2,
  "wiki_updated": 1,
  "graph_nodes_added": 12,
  "graph_edges_added": 34,
  "pushed": null,
  "partial_failure": "push: <error>"
}
```

### 개발 모드

```bash
just dev    # Vite dev server (5173) + axum (8080) 동시 실행
```

`just dev`는 Vite를 5173에서 띄우고 axum이 8080으로 reverse proxy합니다.
- **8080 접속**: 단일 포트로 모든 것 동작 (HMR은 새로고침 필요)
- **5173 직접 접속**: HMR 동작, `/api/*`는 8080으로 프록시됨

### 빌드

```bash
just build          # web/dist 빌드 + cargo build --release
# 또는 수동:
cd web && pnpm install && pnpm build && cd ..
cargo build --release
```

### 사전 요구사항 (개발 시)

- Node 22 + pnpm 9 — `corepack enable` 또는 `npm i -g pnpm`
- [just](https://just.systems) — `brew install just` (선택, 명령 통합용)

## 사용법

### 세션 조회

```bash
# 요약 보기
secall get <session-id>

# 전체 마크다운
secall get <session-id> --full

# 특정 턴
secall get <session-id>:5
```

### 임베딩 생성

시맨틱 검색(`--vec`)을 사용하려면 벡터 인덱스가 필요합니다. Ollama가 설치되어 있으면 `secall embed` 또는 `secall sync` 실행 시 자동으로 임베딩됩니다.

```bash
# 신규/변경된 세션만 임베딩
secall embed

# 전체 재임베딩
secall embed --all

# 성능 옵션 (M1 Max 기준 권장값)
secall embed --concurrency 4 --batch-size 32
```

> ONNX Runtime을 사용하려면 `secall config set embedding.backend ort` 후 `secall model download`로 모델을 다운로드하세요.

### 세션 분류

config에서 정의한 regex 규칙으로 수집 시 세션을 자동 태깅합니다:

```toml
[ingest.classification]
default = "interactive"
skip_embed_types = ["automated"]   # 이 타입은 벡터 임베딩 생략

[[ingest.classification.rules]]
pattern = "^\\[당월 rawdata\\]"
session_type = "automated"

[[ingest.classification.rules]]
pattern = "^# Wiki Incremental Update Prompt"
session_type = "automated"
```

- **수집 시 자동 분류** — 첫 번째 user turn 내용을 rules 순서대로 매칭 (첫 번째 매칭 적용)
- **임베딩 선택적 스킵** — `skip_embed_types`에 지정된 타입은 벡터 임베딩 생략으로 비용 절감
- **검색 필터** — `recall` 및 MCP `recall` 도구가 기본적으로 `automated` 세션 제외 (`--include-automated` 플래그로 포함 가능)
- **소급 분류** — `secall classify --dry-run` / `secall classify`로 기존 세션 일괄 재분류

### 위키 생성

```bash
# Claude Code로 위키 업데이트 (기본값)
secall wiki update

# Codex로 위키 업데이트
secall wiki update --backend codex

# 로컬 LLM 백엔드 사용
secall wiki update --backend ollama
secall wiki update --backend lmstudio

# Codex CLI 백엔드
secall wiki update --backend codex

# Gemini 백엔드
secall wiki update --backend gemini

# 특정 세션만 증분 업데이트
secall wiki update --backend lmstudio --session <id>

# 위키 상태 확인
secall wiki status
```

백엔드는 config로도 설정할 수 있습니다:

```toml
[wiki]
default_backend = "lmstudio"   # "claude" | "codex" | "ollama" | "lmstudio" | "gemini"

[wiki.backends.lmstudio]
api_url = "http://localhost:1234"
model = "lmstudio-community/gemma-4-e4b-it"
max_tokens = 3000

[wiki.backends.ollama]
api_url = "http://localhost:11434"
model = "gemma3:27b"

[wiki.backends.claude]
model = "sonnet"   # "opus" 도 가능
```

### 작업 일기

날짜별 작업 일기를 자동으로 생성합니다:

```bash
# 오늘 날짜 일기 생성
secall log

# 특정 날짜 지정
secall log 2026-04-15
```

- 프로젝트별로 세션을 그룹핑하고, 토픽 노드를 Knowledge Graph에서 추출
- Ollama/Gemini LLM으로 산문 정리 (LLM 미설정 시 템플릿 fallback)
- 결과를 `vault/log/{date}.md`에 저장

### Knowledge Graph

```bash
# 전체 그래프 빌드
secall graph build

# 통계 확인
secall graph stats

# graph.json 내보내기
secall graph export
```

## 설정

`secall config` 명령으로 설정을 관리합니다. config.toml을 직접 편집할 필요가 없습니다.

```bash
# 현재 설정 확인
secall config show

# 설정 변경
secall config set output.timezone Asia/Seoul
secall config set search.tokenizer kiwi
secall config set embedding.backend ollama

# 설정 파일 경로 확인
secall config path
```

### 설정 키 목록

| 키 | 설명 | 기본값 |
|---|---|---|
| `vault.path` | Obsidian vault 경로 | `~/obsidian-vault/seCall` |
| `vault.git_remote` | Git remote URL | (없음) |
| `vault.branch` | Git 브랜치 이름 | `main` |
| `search.tokenizer` | 토크나이저 (`lindera` / `kiwi`) | `lindera` |
| `search.default_limit` | 검색 결과 수 | `10` |
| `embedding.backend` | 임베딩 백엔드 (`ollama` / `ort` / `none`) | `ollama` |
| `embedding.ollama_model` | Ollama 모델 이름 | `bge-m3` |
| `output.timezone` | 타임존 (IANA) | `UTC` |
| `ingest.classification.default` | 분류 규칙 미매칭 시 기본 session_type | `interactive` |
| `ingest.classification.skip_embed_types` | 임베딩을 스킵할 session_type 목록 | `[]` |
| `graph.semantic_backend` | 시맨틱 엣지 추출 백엔드 (`gemini` / `ollama` / `lmstudio` / `none`) | `none` |
| `graph.gemini_model` | Gemini 모델 이름 | `gemini-2.5-flash` |
| `wiki.default_backend` | 위키 생성 백엔드 (`claude` / `codex` / `ollama` / `lmstudio` / `gemini`) | `claude` |
| `wiki.backends.<name>.api_url` | 백엔드 API 엔드포인트 | (기본값 사용) |
| `wiki.backends.<name>.model` | 백엔드 모델 이름 | (기본값 사용) |
| `wiki.backends.<name>.max_tokens` | 최대 생성 토큰 수 | `4096` |

설정 파일 경로:
- **macOS**: `~/Library/Application Support/secall/config.toml`
- **Linux**: `~/.config/secall/config.toml`
- **Windows**: `%APPDATA%\secall\config.toml`

## CLI 레퍼런스

| 명령 | 설명 |
|---|---|
| `secall init` | 대화형 온보딩 (vault, 토크나이저, 임베딩 설정) |
| `secall ingest [path] --auto [--auto-graph]` | 에이전트 세션 파싱 및 인덱싱 (`--auto-graph`로 graph 자동 증분 활성, 기본 비활성) |
| `secall sync [--local-only] [--no-wiki] [--no-semantic] [--no-graph]` | 전체 동기화: init → pull → reindex → ingest → wiki_update → graph → push (`--no-graph`로 graph 단계 생략) |
| `secall recall <query>` | 하이브리드 검색 (기본: automated 세션 제외) |
| `secall recall <query> --include-automated` | automated 세션 포함하여 검색 |
| `secall get <id> [--full]` | 세션 상세 조회 |
| `secall status` | 인덱스 통계 + 설정 요약 |
| `secall embed [--all]` | 벡터 임베딩 생성 |
| `secall classify [--dry-run]` | config 규칙으로 기존 세션 일괄 재분류 |
| `secall lint` | 인덱스/볼트 정합성 검증 |
| `secall mcp [--http <addr>]` | MCP 서버 시작 |
| `secall config show\|set\|path` | 설정 확인/변경 |
| `secall graph build\|stats\|export` | Knowledge Graph 관리 |
| `secall wiki update [--backend claude\|codex\|ollama\|lmstudio\|gemini]` | 위키 생성 (백엔드 선택 가능) |
| `secall wiki status` | 위키 상태 확인 |
| `secall log [YYYY-MM-DD]` | 날짜별 작업 일기 생성 |
| `secall serve [--port <port>]` | REST API 서버 시작 (기본: 8080) |
| `secall model download\|info\|check` | ONNX 모델 관리 |
| `secall reindex --from-vault` | 볼트에서 DB 재구축 |
| `secall migrate summary` | summary frontmatter 일괄 추가 |

## MCP 연동

Claude Code 설정 (`~/.claude/settings.json`)에 추가:

```json
{
  "mcpServers": {
    "secall": {
      "command": "secall",
      "args": ["mcp"]
    }
  }
}
```

세션 시작/종료 시 자동 동기화:

```json
{
  "hooks": {
    "SessionStart": [{
      "matcher": "startup|resume",
      "hooks": [{"type": "command", "command": "secall sync --local-only"}]
    }],
    "SessionEnd": [{
      "hooks": [{"type": "command", "command": "secall sync"}]
    }]
  }
}
```

> 자세한 설정 안내는 [GitHub 볼트 동기화 가이드](docs/reference/github-vault-sync.md)를 참고하세요.

## 아키텍처

```
┌─────────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
│  Claude Code │  │ Codex CLI │  │Gemini CLI│  │claude.ai │  │ ChatGPT  │
│    (JSONL)   │  │  (JSONL)  │  │  (JSON)  │  │JSON (ZIP)│  │JSON (ZIP)│
└──────┬───────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
       │               │             │              │              │
       └───────┬───────┴─────────────┴──────────────┴──────────────┘
               │
         ┌─────▼──────┐
         │   파서들     │  claude.rs / codex.rs / gemini.rs / claude_ai.rs / chatgpt.rs
         └─────┬──────┘
                    │
          ┌─────────▼─────────┐
          │   통합 세션 모델    │  Session → Turn → Action
          └─────────┬─────────┘
                    │
       ┌────────────┼────────────┐
       │            │            │
  ┌────▼────┐ ┌────▼────┐ ┌────▼────┐
  │ SQLite  │ │  볼트   │ │  벡터   │
  │  FTS5   │ │  (MD)   │ │  스토어 │
  │  BM25   │ │Obsidian │ │ BGE-M3  │
  └────┬────┘ └─────────┘ └────┬────┘
       │                       │
       └───────────┬───────────┘
                   │
            ┌──────▼──────┐
            │ 하이브리드 RRF │  k=60
            └──────┬──────┘
                   │
          ┌────────┼────────┐
          │        │        │
     ┌────▼──┐ ┌──▼───┐ ┌──▼──┐
     │  CLI  │ │ MCP  │ │위키 │
     │recall │ │서버   │ │에이전트│
     └───────┘ └──────┘ └─────┘
```

## 기술 스택

| 분류 | 기술 |
|---|---|
| 언어 | Rust 1.75+ (2021 에디션) |
| 데이터베이스 | SQLite + FTS5 (rusqlite, bundled) |
| 한국어 NLP | Lindera ko-dic + Kiwi-rs 형태소 분석 (macOS/Linux) |
| 플랫폼 | macOS, Windows (x86_64), Linux (CI) |
| 임베딩 | Ollama BGE-M3 (1024차원) / ONNX Runtime (선택) |
| ANN 인덱스 | usearch HNSW (macOS/Linux) |
| MCP 서버 | rmcp (stdio + Streamable HTTP / axum) |
| 볼트 | Obsidian 호환 Markdown |
| REST API | axum (CORS 지원) |
| 위키 엔진 | Claude Code / Codex CLI / Ollama / LM Studio / Gemini (플러그인 방식 백엔드) |
| Obsidian 플러그인 | obsidian-secall (TypeScript, esbuild) |

## 출처

이 프로젝트는 다음 아이디어와 프로젝트를 기반으로 합니다:

- **[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)** (Andrej Karpathy) — LLM을 사용하여 원본 소스로부터 점진적으로 지식 베이스를 구축하는 패턴. seCall의 2계층 볼트 아키텍처(원본 세션 + AI 생성 위키)는 이 컨셉을 직접 구현한 것입니다. [Tobi Lütke의 구현](https://github.com/tobi/llm-wiki)도 참고.
- **[qmd](https://github.com/tobi/qmd)** (Tobi Lütke) — 마크다운 파일을 위한 로컬 검색 엔진. seCall의 검색 파이프라인(FTS5 BM25, 벡터 임베딩, RRF k=60)은 qmd의 접근 방식을 참고하여 설계되었습니다.
- **[graphify](https://github.com/safishamsi/graphify)** (Safi Shamsi) — 파일 폴더를 knowledge graph로 변환하는 도구. seCall P16의 결정적 그래프 추출과 confidence 라벨링은 이 프로젝트에서 영감을 받았습니다.

이 프로젝트는 AI 코딩 에이전트(Claude Code, Codex)를 [tunaFlow](https://github.com/hang-in/tunaFlow) 멀티에이전트 워크플로우 플랫폼으로 오케스트레이션하여 개발되었습니다.

## 라이선스

[AGPL-3.0](LICENSE)

## 업데이트 이력

| 날짜 | 버전 | 변경사항 |
|------|------|---------|
| 2026-05-02 | v0.7.0 | Job Cancellation (P36): `tokio_util::sync::CancellationToken` 통합 (`JobRegistry`/`JobExecutor`/`BroadcastSink`), `ProgressSink::is_cancelled()` 추가, sync/ingest/wiki 어댑터 safe-point polling (phase 사이·file/session 루프·LLM 호출 직전), 부분 결과 보존, `POST /api/jobs/{id}/cancel` 활성화 (200 idempotent / 404 unknown, 최종 이벤트 `Failed { error: "cancelled by user" }` + status=`Interrupted`), web UI 취소 버튼 (`JobBanner`/`JobItem`, `useCancelJob` + `window.confirm`) |
| 2026-05-02 | v0.6.0 | Web UI Phase 3 (P35): `/api/tags` 엔드포인트 (with_counts 옵션, 100세션 휴리스틱 제거), SessionList 무한 스크롤 (IntersectionObserver, page_size=100), Code-split (vendor react/query/radix/viz + per-route chunk, 초기 진입 JS ≤ 250 kB gzip) |
| 2026-05-02 | v0.5.0 | Web UI Phase 2 (P34): 시맨틱 검색 모드 활성, 검색어 하이라이트, 다중 태그 + 날짜 quick range, 키보드 단축키 (`?`/`/`/`j`/`k`/`[`/`]`/`g d/w/s/c/g`/`f`/`e`), 관련 세션 패널, 그래프 시각화 강화 (dagre + 노드 색상/아이콘 + 범례), 세션 메타 mini-chart, 사용자 노트 편집 (`PATCH /api/sessions/{id}/notes`), DB 스키마 v7 |
| 2026-05-02 | v0.4.0 | Web UI Phase 1 (P33): 명령 트리거 (Sync/Ingest/Wiki Update), SSE 진행 스트리밍 (phase별), Job 시스템 (단일 큐 + 7일 cleanup + interrupted 보정), 글로벌 진행 배너 + toast, 그래프 자동 증분 (`secall ingest --auto-graph`, `secall sync --no-graph`), 위키 본문 GET 엔드포인트 (`/api/wiki/{project}`), DB v6 (`jobs` 테이블) |
| 2026-04-17 | v0.3.3 | LM Studio (OpenAI 호환) 시맨틱 백엔드 추가 (`--backend lmstudio`, #35), `secall sync --no-semantic` 플래그 추가 — GPU 메모리 경합 방지 (#34), Gemini Web ZIP ingest 지원 (#31), `graph semantic` CLI 백엔드 설정 옵션 (#30) |
| 2026-04-15 | v0.3.2 | Gemini API 백엔드 (시맨틱 그래프 + 일기 생성), Codex wiki 백엔드 (PR #29), REST API 서버 (`secall serve`), Obsidian 플러그인 (검색/데일리/그래프 뷰), 작업 일기 (`secall log`), 시맨틱 엣지 (`fixes_bug`, `modifies_file`, `introduces_tech`, `discusses_topic`), BM25-only 모드 시 graph semantic 자동 비활성화 (#25) |
| 2026-04-12 | v0.3.1 | `secall lint --fix` stale DB 정리 (#15), `wiki_search` created/updated 필드 (#13), P20 테스트 커버리지 강화 (+16 tests) |
| 2026-04-12 | v0.3.0 | 세션 분류 (regex 규칙, `secall classify`), 위키 플러그인 백엔드 (Ollama, LM Studio), `--include-automated` 플래그 |
| 2026-04-10 | P17 | 대화형 온보딩 (`secall init` 위저드), `secall config` CLI, git 브랜치 설정 |
| 2026-04-10 | P16 | Knowledge Graph — frontmatter 기반 결정적 그래프 추출, `secall graph build/stats/export`, MCP `graph_query`, sync Phase 3.7 |
| 2026-04-09 | P15 | Windows 런타임 수정 — Ollama NaN 허용, 크로스플랫폼 `command_exists`, sync 충돌 사전 검사 |
| 2026-04-09 | P14 | 검색 품질 — 독립 벡터 실행, 세션 레벨 결과 다양성 |
| 2026-04-09 | P13 | Windows 빌드 지원 — `x86_64-pc-windows-msvc` CI/Release, ORT DLL 번들 |
| 2026-04-09 | v0.2.3 | ChatGPT 내보내기 파서 — `conversations.json` (ZIP), 매핑 트리 선형화 |
| 2026-04-08 | v0.2.2 | 타임존 설정 — IANA 타임존 변환으로 볼트 타임스탬프 현지화 |
| 2026-04-08 | v0.2.1 | `--force` 재수집 + Dataview `::` 이스케이프 + AGPL-3.0 LICENSE |
| 2026-04-07 | P11 | 임베딩 성능 — ORT 세션 풀, 배치 추론, 병렬화 (49h → ~3-4h) |
| 2026-04-07 | P10 | 세션 `summary` frontmatter — 첫 번째 user turn에서 자동 생성 |
| 2026-04-06 | P8 | 안정화 + GitHub Actions 릴리즈 워크플로우 |
| 2026-04-06 | P7 | `--min-turns`, `embed --all`, `wiki_search` MCP 도구, `--no-wiki` |
| 2026-04-05 | v0.2 | claude.ai 내보내기 파서, ZIP 자동 압축 해제 |
| 2026-04-05 | P6 | ANN 인덱스 (usearch HNSW) |
| 2026-04-04 | P5 | 멀티 기기 볼트 Git 동기화, `secall sync`, `reindex --from-vault` |
| 2026-03-31 | MVP | 최초 릴리즈 — Claude Code/Codex/Gemini 파서, BM25+벡터 검색, MCP 서버, Obsidian 볼트 |

---

<div align="center">

**Contact**: [d9ng@outlook.com](mailto:d9ng@outlook.com)

</div>
