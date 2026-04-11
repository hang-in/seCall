<div align="center">

# seCall

AI 에이전트와 나눈 모든 대화를 검색하세요.

**Search everything you've ever discussed with AI agents.**

[![Rust](https://img.shields.io/badge/Rust-1.75+-f74c00?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5-003B57?logo=sqlite&logoColor=white)](https://www.sqlite.org/)
[![MCP](https://img.shields.io/badge/MCP-Protocol-5A67D8?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIgdmlld0JveD0iMCAwIDI0IDI0Ij48Y2lyY2xlIGN4PSIxMiIgY3k9IjEyIiByPSIxMCIgZmlsbD0id2hpdGUiLz48L3N2Zz4=)](https://modelcontextprotocol.io/)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![ONNX Runtime](https://img.shields.io/badge/ONNX-Runtime-007CFF?logo=onnx&logoColor=white)](https://onnxruntime.ai/)
[![Obsidian](https://img.shields.io/badge/Obsidian-Vault-7C3AED?logo=obsidian&logoColor=white)](https://obsidian.md/)

<br/>

**`한국어`** · [**`English`**](#en) · [**`日本語`**](README.ja.md) · [**`中文`**](README.zh.md)

</div>

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

- **위키 생성**: Claude Code 메타에이전트 기반 (`secall wiki update`)
- **Obsidian 백링크** (`[[]]`)로 세션 ↔ 위키 페이지 연결
- Dataview 쿼리를 위한 frontmatter 메타데이터 (`summary` 필드로 세션 내용 즉시 파악)

### Knowledge Graph

세션 간 관계를 결정적으로 추출하여 지식 그래프를 구축합니다 (LLM 호출 불필요):

- **노드 타입**: session, project, agent, tool — frontmatter에서 자동 추출
- **엣지 타입**: `belongs_to`, `by_agent`, `uses_tool`, `same_project`, `same_day`
- **증분 빌드**: 신규 세션만 노드 추가, 관계 엣지는 전체 재계산으로 정확성 보장
- **MCP 도구**: `graph_query` — AI 에이전트가 세션 간 관계를 탐색 (BFS, 최대 3홉)

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

**소스 빌드:**

```bash
git clone https://github.com/hang-in/seCall.git
cd seCall
cargo install --path crates/secall
```

**사전 빌드 바이너리** ([Releases](https://github.com/hang-in/seCall/releases)):
- macOS: `secall-aarch64-apple-darwin.tar.gz` / `secall-x86_64-apple-darwin.tar.gz`
- Windows: `secall-x86_64-pc-windows-msvc.zip` (secall.exe + onnxruntime.dll)

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

### 위키 생성

```bash
# Claude Code가 세션을 분석하고 위키 페이지를 생성
secall wiki update

# 위키 상태 확인
secall wiki status
```

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

설정 파일 경로:
- **macOS**: `~/Library/Application Support/secall/config.toml`
- **Linux**: `~/.config/secall/config.toml`
- **Windows**: `%APPDATA%\secall\config.toml`

## CLI 레퍼런스

| 명령 | 설명 |
|---|---|
| `secall init` | 대화형 온보딩 (vault, 토크나이저, 임베딩 설정) |
| `secall ingest [path] --auto` | 에이전트 세션 파싱 및 인덱싱 |
| `secall sync [--local-only] [--no-wiki]` | 전체 동기화: git pull → reindex → ingest → wiki → graph → git push |
| `secall recall <query>` | 하이브리드 검색 |
| `secall get <id> [--full]` | 세션 상세 조회 |
| `secall status` | 인덱스 통계 + 설정 요약 |
| `secall embed [--all]` | 벡터 임베딩 생성 |
| `secall lint` | 인덱스/볼트 정합성 검증 |
| `secall mcp [--http <addr>]` | MCP 서버 시작 |
| `secall config show\|set\|path` | 설정 확인/변경 |
| `secall graph build\|stats\|export` | Knowledge Graph 관리 |
| `secall wiki update\|status` | 위키 생성/상태 확인 |
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
    "PreToolUse": [{
      "matcher": "Initialize",
      "hooks": [{"type": "command", "command": "secall sync --local-only"}]
    }],
    "PostToolUse": [{
      "matcher": "Exit",
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
| 위키 엔진 | Claude Code 메타에이전트 |

## 출처

이 프로젝트는 다음 아이디어와 프로젝트를 기반으로 합니다:

- **[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)** (Andrej Karpathy) — LLM을 사용하여 원본 소스로부터 점진적으로 지식 베이스를 구축하는 패턴. seCall의 2계층 볼트 아키텍처(원본 세션 + AI 생성 위키)는 이 컨셉을 직접 구현한 것입니다. [Tobi Lütke의 구현](https://github.com/tobi/llm-wiki)도 참고.
- **[qmd](https://github.com/tobi/qmd)** (Tobi Lütke) — 마크다운 파일을 위한 로컬 검색 엔진. seCall의 검색 파이프라인(FTS5 BM25, 벡터 임베딩, RRF k=60)은 qmd의 접근 방식을 참고하여 설계되었습니다.
- **[graphify](https://github.com/safishamsi/graphify)** (Safi Shamsi) — 파일 폴더를 knowledge graph로 변환하는 도구. seCall P16의 결정적 그래프 추출과 confidence 라벨링은 이 프로젝트에서 영감을 받았습니다.

이 프로젝트는 AI 코딩 에이전트(Claude Code, Codex)를 [tunaFlow](https://github.com/hang-in/tunaFlow) 멀티에이전트 워크플로우 플랫폼으로 오케스트레이션하여 개발되었습니다.

## 라이선스

[AGPL-3.0](LICENSE)

---

<a id="en"></a>

<div align="center">

[**`한국어`**](#secall) · **`English`** · [**`日本語`**](README.ja.md) · [**`中文`**](README.zh.md)

</div>

<div align="center">
<img src="screenshot.png" alt="seCall Obsidian Vault" width="720" />
<br/><br/>
</div>

## What is seCall?

seCall is a local-first search engine for AI agent sessions. It ingests conversation logs from **Claude Code**, **Codex CLI**, **Gemini CLI**, **claude.ai**, and **ChatGPT**, indexes them with hybrid BM25 + vector search, and exposes them via CLI, MCP server, and an Obsidian-compatible knowledge vault.

Your AI conversations are a knowledge base. seCall makes them searchable, browsable, and interconnected.

### Why?

- You've discussed architecture, debugging steps, and design decisions across hundreds of agent sessions — but they're scattered in opaque JSONL files.
- seCall turns those sessions into a **structured, searchable knowledge graph** you can query from any MCP-compatible AI agent or browse in Obsidian.

## Features

### Multi-Agent Ingestion

Parse and normalize sessions from multiple AI coding agents into a unified format:

| Agent | Format | Status |
|---|---|---|
| Claude Code | JSONL | ✅ Stable |
| Codex CLI | JSONL | ✅ Stable |
| Gemini CLI | JSON | ✅ Stable |
| claude.ai | JSON (ZIP) | ✅ New in v0.2 |
| ChatGPT | JSON (ZIP) | ✅ New in v0.2.3 |

### Hybrid Search

- **BM25 full-text search** powered by SQLite FTS5 with Korean morpheme tokenization ([Lindera](https://github.com/lindera/lindera) ko-dic / [Kiwi-rs](https://github.com/bab2min/kiwi) selectable)
- **Vector semantic search** using [Ollama](https://ollama.com/) BGE-M3 embeddings (1024-dim) + **HNSW ANN index** ([usearch](https://github.com/unum-cloud/usearch)) for O(log n) lookups
- **Reciprocal Rank Fusion (RRF)** with independent BM25/vector execution (k=60) + **session-level diversity** (max 2 turns per session)
- **LLM query expansion** for natural language queries via Claude Code

### Knowledge Vault

Obsidian-compatible markdown vault with two layers:

```
vault/
├── raw/sessions/    # Immutable session transcripts
│   └── YYYY-MM-DD/  # Organized by date
├── wiki/            # AI-generated knowledge pages
│   ├── projects/    # Per-project summaries
│   ├── topics/      # Technical topic pages
│   └── decisions/   # Architecture decision records
└── graph/           # Knowledge Graph output
    └── graph.json   # Node/edge data
```

- **Wiki generation** via Claude Code meta-agent (`secall wiki update`)
- **Obsidian backlinks** (`[[]]`) connecting sessions ↔ wiki pages
- Frontmatter metadata for Dataview queries (`summary` field for at-a-glance session identification)

### Knowledge Graph

Deterministically extract relationships between sessions to build a knowledge graph (no LLM calls needed):

- **Node types**: session, project, agent, tool — auto-extracted from frontmatter
- **Edge types**: `belongs_to`, `by_agent`, `uses_tool`, `same_project`, `same_day`
- **Incremental builds**: new sessions get nodes added; relation edges are fully recomputed for accuracy
- **MCP tool**: `graph_query` — AI agents can explore session relationships (BFS, max 3 hops)

### MCP Server

Expose your session index to any MCP-compatible AI agent:

```bash
# stdio mode (for Claude Code, Cursor, etc.)
secall mcp

# HTTP mode (for web clients)
secall mcp --http 127.0.0.1:8080
```

Tools provided: `recall`, `get`, `status`, `wiki_search`, `graph_query`

### Multi-Device Vault Sync

Sync your knowledge vault across machines via Git:

```bash
# Full sync: git pull → reindex → ingest → wiki → graph → git push
secall sync

# Local-only mode (skip git, useful for Claude Code hooks)
secall sync --local-only
```

- **MD as source of truth** — DB is a derived cache, fully recoverable via `secall reindex --from-vault`
- **Host tracking** — each session records which machine ingested it (`host` field in frontmatter)
- **No conflicts** — sessions are unique per device, so git merges are always clean

### Data Integrity

Built-in lint rules verify index ↔ vault consistency:

```bash
secall lint
# L001: Missing vault files
# L002: Orphan vault files
# L003: FTS index gaps
```

## Quick Start

### Prerequisites

- Rust 1.75+ (for building from source)
- At least one of: Claude Code, Codex CLI, Gemini CLI
- [Ollama](https://ollama.com/) — for vector search (optional; BM25-only without it)
- **Windows**: MSVC toolchain (Visual Studio Build Tools)

### Step 1. Install

**From source:**

```bash
git clone https://github.com/hang-in/seCall.git
cd seCall
cargo install --path crates/secall
```

**Pre-built binaries** ([Releases](https://github.com/hang-in/seCall/releases)):
- macOS: `secall-aarch64-apple-darwin.tar.gz` / `secall-x86_64-apple-darwin.tar.gz`
- Windows: `secall-x86_64-pc-windows-msvc.zip` (secall.exe + onnxruntime.dll)

> **Windows users**: Core features (parsing, BM25 search, vault, MCP) work identically. The following are disabled due to MSVC limitations:
> - **HNSW ANN index** (`usearch`) — falls back to BLOB cosine scan
> - **Kiwi-rs morpheme analysis** — falls back to Lindera ko-dic

### Step 2. Initialize

```bash
# Interactive onboarding (recommended)
secall init

# Or specify arguments directly
secall init --vault ~/Documents/Obsidian\ Vault/seCall
secall init --git git@github.com:you/obsidian-vault.git
```

Running `secall init` without arguments starts an interactive wizard:
- Vault path setup
- Git remote (optional)
- Tokenizer selection (lindera/kiwi)
- Embedding backend selection (ollama/none)
- Ollama installation check + automatic `bge-m3` model pull

### Step 3. Ingest Sessions

```bash
# Auto-detect Claude Code sessions
secall ingest --auto

# Codex CLI / Gemini CLI
secall ingest ~/.codex/sessions
secall ingest ~/.gemini/sessions

# claude.ai / ChatGPT export (ZIP)
secall ingest ~/Downloads/data-export.zip

# Or sync everything in one command
secall sync
```

### Step 4. Search

```bash
# BM25 full-text search
secall recall "BM25 indexing implementation"

# Filter by project, agent, date
secall recall "error handling" --project seCall --agent claude-code --since 2026-04-01

# Vector semantic search (requires Ollama)
secall recall "how does the search pipeline work" --vec

# LLM-expanded query
secall recall "improve search accuracy" --expand
```

## Usage

### Retrieve a Session

```bash
# Summary view
secall get <session-id>

# Full markdown content
secall get <session-id> --full

# Specific turn
secall get <session-id>:5
```

### Build Embeddings

For semantic search (`--vec`), vector indexes are needed. With Ollama installed, `secall embed` or `secall sync` will generate embeddings automatically.

```bash
# Embed new/changed sessions only
secall embed

# Re-embed all sessions
secall embed --all

# Performance tuning (recommended for M1 Max)
secall embed --concurrency 4 --batch-size 32
```

> To use ONNX Runtime instead: `secall config set embedding.backend ort` then `secall model download`.

### Generate Wiki

```bash
# Claude Code analyzes sessions and generates wiki pages
secall wiki update

# Check wiki status
secall wiki status
```

### Knowledge Graph

```bash
# Build entire graph
secall graph build

# View statistics
secall graph stats

# Export graph.json
secall graph export
```

## Configuration

Manage settings via the `secall config` command. No need to edit config.toml directly.

```bash
# View current settings
secall config show

# Change a setting
secall config set output.timezone Asia/Seoul
secall config set search.tokenizer kiwi
secall config set embedding.backend ollama

# Show config file path
secall config path
```

### Available Keys

| Key | Description | Default |
|---|---|---|
| `vault.path` | Obsidian vault path | `~/obsidian-vault/seCall` |
| `vault.git_remote` | Git remote URL | (none) |
| `vault.branch` | Git branch name | `main` |
| `search.tokenizer` | Tokenizer (`lindera` / `kiwi`) | `lindera` |
| `search.default_limit` | Search result count | `10` |
| `embedding.backend` | Embedding backend (`ollama` / `ort` / `none`) | `ollama` |
| `embedding.ollama_model` | Ollama model name | `bge-m3` |
| `output.timezone` | Timezone (IANA) | `UTC` |

Config file location:
- **macOS**: `~/Library/Application Support/secall/config.toml`
- **Linux**: `~/.config/secall/config.toml`
- **Windows**: `%APPDATA%\secall\config.toml`

## CLI Reference

| Command | Description |
|---|---|
| `secall init` | Interactive onboarding (vault, tokenizer, embedding setup) |
| `secall ingest [path] --auto` | Parse and index agent sessions |
| `secall sync [--local-only] [--no-wiki]` | Full sync: git pull → reindex → ingest → wiki → graph → git push |
| `secall recall <query>` | Hybrid search across sessions |
| `secall get <id> [--full]` | Retrieve session details |
| `secall status` | Index statistics + settings summary |
| `secall embed [--all]` | Generate vector embeddings |
| `secall lint` | Verify index/vault integrity |
| `secall mcp [--http <addr>]` | Start MCP server |
| `secall config show\|set\|path` | View/change settings |
| `secall graph build\|stats\|export` | Knowledge graph management |
| `secall wiki update\|status` | Wiki generation/status |
| `secall model download\|info\|check` | ONNX model management |
| `secall reindex --from-vault` | Rebuild DB from vault |
| `secall migrate summary` | Backfill summary frontmatter |

## MCP Integration

Add to your Claude Code settings (`~/.claude/settings.json`):

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

For auto-sync on session start/end:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Initialize",
      "hooks": [{"type": "command", "command": "secall sync --local-only"}]
    }],
    "PostToolUse": [{
      "matcher": "Exit",
      "hooks": [{"type": "command", "command": "secall sync"}]
    }]
  }
}
```

> See [GitHub Vault Sync Guide](docs/reference/github-vault-sync.md) for detailed setup instructions.

## Architecture

```
┌─────────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
│  Claude Code │  │ Codex CLI │  │Gemini CLI│  │claude.ai │  │ ChatGPT  │
│    (JSONL)   │  │  (JSONL)  │  │  (JSON)  │  │JSON (ZIP)│  │JSON (ZIP)│
└──────┬───────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
       │               │             │              │              │
       └───────┬───────┴─────────────┴──────────────┴──────────────┘
               │
         ┌─────▼──────┐
         │   Parsers   │  claude.rs / codex.rs / gemini.rs / claude_ai.rs / chatgpt.rs
         └─────┬──────┘
                    │
          ┌─────────▼─────────┐
          │   Unified Session  │  Session → Turn → Action
          └─────────┬─────────┘
                    │
       ┌────────────┼────────────┐
       │            │            │
  ┌────▼────┐ ┌────▼────┐ ┌────▼────┐
  │ SQLite  │ │  Vault  │ │  Vector │
  │  FTS5   │ │   (MD)  │ │  Store  │
  │  BM25   │ │Obsidian │ │BGE-M3   │
  └────┬────┘ └─────────┘ └────┬────┘
       │                       │
       └───────────┬───────────┘
                   │
            ┌──────▼──────┐
            │  Hybrid RRF  │  k=60
            └──────┬──────┘
                   │
          ┌────────┼────────┐
          │        │        │
     ┌────▼──┐ ┌──▼───┐ ┌──▼──┐
     │  CLI  │ │ MCP  │ │Wiki │
     │recall │ │Server│ │Agent│
     └───────┘ └──────┘ └─────┘
```

## Tech Stack

| Category | Technology |
|---|---|
| Language | Rust 1.75+ (2021 edition) |
| Database | SQLite with FTS5 (rusqlite, bundled) |
| Korean NLP | Lindera ko-dic + Kiwi-rs morpheme analysis (macOS/Linux) |
| Platforms | macOS, Windows (x86_64), Linux (CI) |
| Embeddings | Ollama BGE-M3 (1024-dim) / ONNX Runtime (optional) |
| ANN Index | usearch HNSW (macOS/Linux) |
| MCP Server | rmcp (stdio + Streamable HTTP via axum) |
| Vault | Obsidian-compatible Markdown |
| Wiki Engine | Claude Code meta-agent |

## Acknowledgments

This project is built on ideas from:

- **[LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)** by Andrej Karpathy — The pattern of using LLMs to incrementally build a persistent, interlinked knowledge base from raw sources. seCall's two-layer vault architecture (raw sessions + AI-generated wiki) directly implements this concept. See also [Tobi Lütke's implementation](https://github.com/tobi/llm-wiki).
- **[qmd](https://github.com/tobi/qmd)** by Tobi Lütke — A local search engine for markdown files with hybrid BM25/vector search. seCall's search pipeline (FTS5 BM25, vector embeddings, RRF k=60) was designed with reference to qmd's approach.
- **[graphify](https://github.com/safishamsi/graphify)** by Safi Shamsi — Turns file folders into queryable knowledge graphs. seCall P16's deterministic graph extraction and confidence labeling were inspired by this project.

This project was developed using AI coding agents (Claude Code, Codex) orchestrated via [tunaFlow](https://github.com/hang-in/tunaFlow), a multi-agent workflow platform.

## License

[AGPL-3.0](LICENSE)

## Updates

| Date | Version | Changes |
|------|---------|---------|
| 2026-04-10 | P17 | Interactive onboarding (`secall init` wizard), `secall config` CLI, git branch configuration |
| 2026-04-10 | P16 | Knowledge Graph — deterministic graph extraction from frontmatter, `secall graph build/stats/export`, MCP `graph_query`, sync Phase 3.7 |
| 2026-04-09 | P15 | Windows runtime fixes — Ollama NaN tolerance, cross-platform `command_exists`, sync conflict preflight |
| 2026-04-09 | P14 | Search quality — independent vector execution, session-level result diversity |
| 2026-04-09 | P13 | Windows build support — `x86_64-pc-windows-msvc` CI/Release, ORT DLL bundling |
| 2026-04-09 | v0.2.3 | ChatGPT export parser — `conversations.json` (ZIP), mapping tree linearization |
| 2026-04-08 | v0.2.2 | Timezone configuration — IANA timezone conversion for vault timestamps |
| 2026-04-08 | v0.2.1 | `--force` re-ingest + Dataview `::` escaping + AGPL-3.0 LICENSE |
| 2026-04-07 | P11 | Embedding performance — ORT session pool, batch inference, parallelism (49h → ~3-4h) |
| 2026-04-07 | P10 | Session `summary` frontmatter — auto-generated from first user turn |
| 2026-04-06 | P8 | Stabilization + GitHub Actions release workflow |
| 2026-04-06 | P7 | `--min-turns`, `embed --all`, `wiki_search` MCP tool, `--no-wiki` |
| 2026-04-05 | v0.2 | claude.ai export parser, ZIP auto-extraction |
| 2026-04-05 | P6 | ANN index (usearch HNSW) |
| 2026-04-04 | P5 | Multi-device vault Git sync, `secall sync`, `reindex --from-vault` |
| 2026-03-31 | MVP | Initial release — Claude Code/Codex/Gemini parsers, BM25+vector search, MCP server, Obsidian vault |

---

<div align="center">

**Contact**: [d9ng@outlook.com](mailto:d9ng@outlook.com)

</div>
