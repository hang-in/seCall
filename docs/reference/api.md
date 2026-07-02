---
type: reference
status: done
updated_at: 2026-07-02
canonical: true
---

# seCall API 레퍼런스 (REST + MCP)

seCall 의 프로그래매틱 인터페이스 두 가지 — **REST API**(Web UI · Obsidian 플러그인 공용)와 **MCP 서버**(MCP 호환 AI 에이전트용) — 의 상세 문서.

> 이 문서는 README 에서 분리된 API 상세다. README 에는 "바로 시작"에 필요한 요약만 두고, 엔드포인트/도구 전체 목록은 여기서 관리한다. 엔드포인트의 최종 기준(SSOT)은 코드(`crates/secall-core/src/mcp/rest.rs`)다.

---

## REST API

`secall serve` 는 REST API 와 Web UI 를 **동일 포트(기본 8080)** 에서 제공한다. Obsidian 플러그인도 같은 API 를 공유한다.

```bash
# REST API + Web UI 서버 시작
secall serve --port 8080
# 브라우저: http://127.0.0.1:8080

# /settings 에서 config 저장을 허용하려면 (기본은 read-only)
secall serve --allow-config-edit
```

### 읽기 / 검색

| 엔드포인트 | 설명 |
| --- | --- |
| `POST /api/recall` | 세션 검색 (BM25 / 벡터 / hybrid) |
| `POST /api/get` | 특정 세션 조회 |
| `GET /api/status` | 인덱스 상태 |
| `POST /api/daily` | 데일리 노트 |
| `POST /api/graph` | Knowledge Graph 조회 |
| `GET /api/graph/snapshot` | Knowledge Graph 스냅샷 (edge_limit + 우선순위 sampling) |
| `POST /api/wiki` | 위키 검색 |
| `GET /api/wiki` | 위키 목록 |
| `GET /api/wiki/{project}` | 위키 본문 |

### 세션 메타

| 엔드포인트 | 설명 |
| --- | --- |
| `GET /api/sessions` | 세션 목록 |
| `GET /api/projects` | 프로젝트 목록 |
| `GET /api/agents` | 에이전트 목록 |
| `GET /api/tags?with_counts={true\|false}` | 태그 목록 (`true`: `{name,count}` / `false`: 이름만) |
| `PATCH /api/sessions/{id}/tags` | 태그 편집 |
| `PATCH /api/sessions/{id}/favorite` | 즐겨찾기 토글 |
| `PATCH /api/sessions/{id}/notes` | 세션 노트 저장 |
| `DELETE /api/sessions/{id}` | 세션 완전 삭제 (#108) |

### 설정

| 엔드포인트 | 설명 |
| --- | --- |
| `GET /api/config` | 설정 조회 |
| `PATCH /api/config/{section}` | 설정 수정 (`--allow-config-edit` 필요, secret 키는 응답/저장에서 필터) |
| `GET /api/models?backend={name}&force={true\|false}` | 백엔드별 사용 가능 모델 목록 |

### 명령 (비동기 Job)

| 엔드포인트 | 설명 |
| --- | --- |
| `POST /api/commands/{sync,ingest,wiki-update}` | 각 작업 트리거 → `{ job_id, status: "started" }` (HTTP 202) |
| `POST /api/commands/graph-rebuild` | 그래프 재구축. body: `{ since?, session?, all?, retry_failed? }` |

> **단일 큐 정책**: 다른 mutating job 실행 중이면 `409 Conflict`. 우선순위(graph-rebuild): `session` > `all` > `retry_failed` > `since`.

### Job

| 엔드포인트 | 설명 |
| --- | --- |
| `GET /api/jobs` | Job 목록 |
| `GET /api/jobs/{id}` | Job 상태 |
| `GET /api/jobs/{id}/stream` | 진행 스트리밍 (SSE) |
| `POST /api/jobs/{id}/cancel` | Job 취소 — `200 {cancelled:true}` (idempotent) / `404` (미등록·evict) |

---

## MCP 서버

MCP 호환 AI 에이전트(Claude Code, Cursor 등)에서 seCall 검색을 사용한다.

```bash
# stdio 모드
secall mcp

# HTTP 모드
secall mcp --http 127.0.0.1:8080
```

### 제공 도구

| 도구 | 설명 |
| --- | --- |
| `recall` | 세션 검색 |
| `get` | 특정 세션 조회 |
| `status` | 인덱스 상태 확인 |
| `wiki_search` | 위키 문서 검색 |
| `graph_query` | Knowledge Graph 탐색 |

### Claude Code 설정 예시

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

### 세션 시작 / 종료 시 자동 동기화 (hooks)

```json
{
  "hooks": {
    "SessionStart": [{
      "matcher": "startup|resume",
      "hooks": [{
        "type": "command",
        "command": "secall sync --local-only"
      }]
    }],
    "SessionEnd": [{
      "hooks": [{
        "type": "command",
        "command": "secall sync"
      }]
    }]
  }
}
```
