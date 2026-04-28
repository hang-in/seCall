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

[**`한국어`**](README.md) · **`English`** · [**`日本語`**](README.ja.md) · [**`中文`**](README.zh.md)

</div>

---

<div align="center">
<img src="screenshot.png" alt="seCall Obsidian Vault" width="720" />
<br/><br/>
</div>

## Table of Contents

- [What is seCall?](#what-is-secall)
- [Features](#features)
  - [Multi-Agent Ingestion](#multi-agent-ingestion)
  - [Hybrid Search](#hybrid-search)
  - [Knowledge Vault](#knowledge-vault)
  - [Knowledge Graph](#knowledge-graph)
  - [REST API + Obsidian Plugin](#rest-api--obsidian-plugin)
  - [MCP Server](#mcp-server)
  - [Multi-Device Vault Sync](#multi-device-vault-sync)
  - [Data Integrity](#data-integrity)
- [Quick Start](#quick-start)
  - [Prerequisites](#prerequisites)
  - [Step 1. Install](#step-1-install)
  - [Step 2. Initialize](#step-2-initialize)
  - [Step 3. Ingest Sessions](#step-3-ingest-sessions)
  - [Step 4. Search](#step-4-search)
- [Usage](#usage)
  - [Retrieve a Session](#retrieve-a-session)
  - [Build Embeddings](#build-embeddings)
  - [Session Classification](#session-classification)
  - [Generate Wiki](#generate-wiki)
  - [Daily Work Log](#daily-work-log)
  - [Knowledge Graph](#knowledge-graph-1)
- [Configuration](#configuration)
  - [Available Keys](#available-keys)
- [CLI Reference](#cli-reference)
- [MCP Integration](#mcp-integration)
- [Architecture](#architecture)
- [Tech Stack](#tech-stack)
- [Acknowledgments](#acknowledgments)
- [License](#license)
- [Updates](#updates)

---

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

- **Wiki generation** via pluggable LLM backends (`secall wiki update --backend claude|codex|ollama|lmstudio`)
- **Obsidian backlinks** (`[[]]`) connecting sessions ↔ wiki pages
- Frontmatter metadata for Dataview queries (`summary` field for at-a-glance session identification)

### Knowledge Graph

Extract relationships between sessions to build a knowledge graph:

- **Node types**: session, project, agent, tool — auto-extracted from frontmatter
- **Rule-based edges**: `belongs_to`, `by_agent`, `uses_tool`, `same_project`, `same_day` (no LLM needed)
- **Semantic edges** (Gemini/Ollama): `fixes_bug`, `modifies_file`, `introduces_tech`, `discusses_topic` — LLM analyzes session content
- **Incremental builds**: new sessions get nodes added; relation edges are fully recomputed for accuracy
- **MCP tool**: `graph_query` — AI agents can explore session relationships (BFS, max 3 hops)

### REST API + Obsidian Plugin

Browse sessions from a REST API server and a dedicated Obsidian plugin:

```bash
# Start REST API server
secall serve --port 8080
```

**Endpoints**: `/api/recall`, `/api/get`, `/api/status`, `/api/daily`, `/api/graph`

**Obsidian Plugin** (`obsidian-secall/`):
- **Search View** — keyword/semantic session search
- **Daily View** — daily work summary grouped by project, with note creation
- **Graph View** — explore node relationships (depth 1-3, relation filters)
- **Session View** — full markdown rendering
- **Status bar** — session count + embedding status (refreshes every 5 min)

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

### Session Classification

Tag sessions automatically during ingest using config-driven regex rules:

```toml
[ingest.classification]
default = "interactive"
skip_embed_types = ["automated"]   # skip vector embedding for these types

[[ingest.classification.rules]]
pattern = "^\\[monthly rawdata\\]"
session_type = "automated"
```

- Rules are matched against the first user turn (first match wins)
- `skip_embed_types` skips vector embedding for cost savings
- `recall` and MCP `recall` exclude `automated` sessions by default (`--include-automated` to override)
- `secall classify [--dry-run]` backfills existing sessions

### Generate Wiki

```bash
# Use Claude Code (default)
secall wiki update

# Use Codex
secall wiki update --backend codex

# Use a local LLM backend
secall wiki update --backend ollama
secall wiki update --backend lmstudio

# Codex CLI backend
secall wiki update --backend codex

# Gemini backend
secall wiki update --backend gemini

# Incremental update for one session
secall wiki update --backend lmstudio --session <id>

# Check wiki status
secall wiki status
```

Configure the default backend in `config.toml`:

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
```

### Daily Work Log

Generate daily work diaries automatically:

```bash
# Generate for today
secall log

# Specify a date
secall log 2026-04-15
```

- Groups sessions by project, extracts topic nodes from Knowledge Graph
- Uses Ollama/Gemini LLM for prose summary (falls back to template without LLM)
- Saves to `vault/log/{date}.md`

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
| `ingest.classification.default` | Default session_type when no rule matches | `interactive` |
| `ingest.classification.skip_embed_types` | Session types to skip vector embedding | `[]` |
| `graph.semantic_backend` | Semantic edge extraction backend (`gemini` / `ollama` / `none`) | `none` |
| `graph.gemini_model` | Gemini model name | `gemini-2.5-flash` |
| `wiki.default_backend` | Wiki generation backend (`claude` / `codex` / `ollama` / `lmstudio` / `gemini`) | `claude` |
| `wiki.backends.<name>.api_url` | Backend API endpoint | (default) |
| `wiki.backends.<name>.model` | Model name for the backend | (default) |
| `wiki.backends.<name>.max_tokens` | Max tokens to generate | `4096` |

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
| `secall recall <query>` | Hybrid search (automated sessions excluded by default) |
| `secall recall <query> --include-automated` | Search including automated sessions |
| `secall get <id> [--full]` | Retrieve session details |
| `secall status` | Index statistics + settings summary |
| `secall embed [--all]` | Generate vector embeddings |
| `secall classify [--dry-run]` | Backfill session types using config rules |
| `secall lint` | Verify index/vault integrity |
| `secall mcp [--http <addr>]` | Start MCP server |
| `secall config show\|set\|path` | View/change settings |
| `secall graph build\|stats\|export` | Knowledge graph management |
| `secall wiki update [--backend claude\|codex\|ollama\|lmstudio\|gemini]` | Wiki generation with backend selection |
| `secall wiki status` | Wiki status |
| `secall log [YYYY-MM-DD]` | Generate daily work diary |
| `secall serve [--port <port>]` | Start REST API server (default: 8080) |
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
| REST API | axum (with CORS) |
| Wiki Engine | Claude Code / Codex CLI / Ollama / LM Studio / Gemini (pluggable backends) |
| Obsidian Plugin | obsidian-secall (TypeScript, esbuild) |

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
| 2026-04-15 | v0.3.2 | Gemini API backend (semantic graph + diary), Codex wiki backend (PR #29), REST API server (`secall serve`), Obsidian plugin (search/daily/graph views), daily work log (`secall log`), semantic edges (`fixes_bug`, `modifies_file`, `introduces_tech`, `discusses_topic`), auto-disable graph semantic in BM25-only mode (#25) |
| 2026-04-12 | v0.3.1 | `secall lint --fix` stale DB cleanup (#15), `wiki_search` created/updated fields (#13), P20 test coverage (+16 tests) |
| 2026-04-12 | v0.3.0 | Session classification (regex rules, `secall classify`), wiki pluggable backends (Ollama, LM Studio), `--include-automated` flag |
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
