# seCall

AI 에이전트와 나눈 대화를 **로컬 위키**로 정리하고 검색하는 도구입니다.

Claude Code, Codex CLI, Gemini CLI, claude.ai, ChatGPT 세션을 수집해 Obsidian 호환 Markdown 볼트로 저장하고, BM25 + 벡터 검색 + MCP + Web UI를 통해 다시 찾아볼 수 있게 합니다.

<div align="center">

[![Rust](https://img.shields.io/badge/Rust-1.75+-f74c00?logo=rust\&logoColor=white)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5-003B57?logo=sqlite\&logoColor=white)](https://www.sqlite.org/)
[![MCP](https://img.shields.io/badge/MCP-Protocol-5A67D8)](https://modelcontextprotocol.io/)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![ONNX Runtime](https://img.shields.io/badge/ONNX-Runtime-007CFF?logo=onnx\&logoColor=white)](https://onnxruntime.ai/)
[![Obsidian](https://img.shields.io/badge/Obsidian-Plugin-7C3AED?logo=obsidian\&logoColor=white)](https://obsidian.md/)

**`한국어`** · [**`English`**](README.en.md) · [**`日本語`**](README.ja.md) · [**`中文`**](README.zh.md)

</div>

---

## seCall이 필요한 이유

AI 에이전트를 오래 쓰다 보면 중요한 작업 기록이 여러 위치에 흩어집니다.

* Claude Code / Codex / Gemini 세션 로그
* ChatGPT / claude.ai export 파일
* 디버깅 과정
* 아키텍처 결정
* 임시 설계 메모
* “지난번에 어떻게 해결했더라?” 싶은 대화

seCall은 이 기록을 로컬에 모으고, 원본 transcript를 보존한 뒤, 그 위에 AI가 정리한 위키를 얹습니다.

검색은 CLI, Web UI, Obsidian, MCP 호환 AI 에이전트에서 모두 사용할 수 있습니다.

---

## 주요 기능

| 기능              | 설명                                                           |
| --------------- | ------------------------------------------------------------ |
| 멀티 에이전트 수집      | Claude Code, Codex CLI, Gemini CLI, claude.ai, ChatGPT 세션 수집 |
| 하이브리드 검색        | SQLite FTS5 BM25 + BGE-M3 벡터 검색 + RRF 결합                     |
| Obsidian 볼트     | 원본 세션과 AI 생성 위키를 Markdown으로 저장                               |
| Knowledge Graph | 세션, 프로젝트, 에이전트, 도구, 토픽 간 관계 추출                               |
| Web UI          | 검색, 세션 브라우징, 그래프 탐색, 명령 실행                                   |
| REST API        | Web UI와 Obsidian 플러그인이 사용하는 동일 API 제공                        |
| MCP 서버          | Claude Code, Cursor 등 MCP 클라이언트에서 과거 세션 검색                   |
| Git 동기화         | 여러 기기에서 같은 볼트를 동기화                                           |
| 데이터 무결성 검사      | DB와 볼트의 정합성 lint 제공                                          |

---

## 지원하는 세션 형식

| 에이전트 / 서비스  | 형식              | 상태 |
| ----------- | --------------- | -- |
| Claude Code | JSONL           | 안정 |
| Codex CLI   | JSONL           | 안정 |
| Gemini CLI  | JSON            | 안정 |
| claude.ai   | JSON ZIP export | 지원 |
| ChatGPT     | JSON ZIP export | 지원 |

---

## 빠른 시작

### 1. 설치

macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/seCall/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/hang-in/seCall/main/install.ps1 | iex
```

Linux는 아직 prebuilt 바이너리가 없습니다. Cargo 빌드를 사용하세요.

```bash
cargo install --path crates/secall --no-default-features
```

웹 UI까지 포함해 직접 빌드하려면 다음 도구가 필요합니다.

* Rust 1.75+
* Node 22
* pnpm 9
* just

```bash
git clone https://github.com/hang-in/seCall.git
cd seCall
just build
cp target/release/secall ~/.local/bin/
```

> `cargo install secall`은 웹 UI 빌드를 자동 수행하지 않습니다. Web UI가 필요하면 Release 바이너리 또는 `just build`를 사용하세요.

---

### 2. 초기화

대화형 초기화:

```bash
secall init
```

직접 경로 지정:

```bash
secall init --vault ~/Documents/Obsidian\ Vault/seCall
secall init --git git@github.com:you/obsidian-vault.git
```

초기화 과정에서 다음 항목을 설정합니다.

| 항목         | 설명                                                            |
| ---------- | ------------------------------------------------------------- |
| Vault 경로   | 세션과 위키가 저장될 Obsidian 호환 Markdown 볼트                           |
| Git remote | 여러 기기 동기화용 원격 저장소                                             |
| 토크나이저      | `lindera` 또는 `kiwi`                                           |
| 임베딩 백엔드    | `ollama`, `ort`, `openai`, `openvino`, `ollama_cloud`, `none` |
| Ollama 모델  | 기본값 `bge-m3`                                                  |

---

### 3. 세션 수집

자동 감지:

```bash
secall ingest --auto
```

에이전트별 세션 경로 지정:

```bash
secall ingest ~/.codex/sessions
secall ingest ~/.gemini/sessions
```

claude.ai / ChatGPT export ZIP 수집:

```bash
secall ingest ~/Downloads/data-export.zip
```

전체 동기화:

```bash
secall sync
```

---

### 4. 검색

기본 BM25 검색:

```bash
secall recall "BM25 인덱싱 구현"
```

프로젝트, 에이전트, 날짜 필터:

```bash
secall recall "에러 처리" \
  --project seCall \
  --agent claude-code \
  --since 2026-04-01
```

벡터 검색:

```bash
secall recall "검색 파이프라인 동작 방식" --vec
```

LLM 쿼리 확장:

```bash
secall recall "검색 정확도 개선" --expand
```

---

## Web UI

REST API와 Web UI는 `secall serve` 하나로 실행합니다.

```bash
secall serve --port 8080
```

브라우저에서 접속:

```text
http://127.0.0.1:8080
```

Web UI에서 할 수 있는 일:

| 기능        | 설명                                          |
| --------- | ------------------------------------------- |
| 세션 검색     | 키워드 / 시맨틱 검색                                |
| 세션 상세 보기  | 원본 transcript Markdown 렌더링                  |
| Daily 보기  | 날짜별 작업 기록                                   |
| Wiki 보기   | AI가 정리한 프로젝트 / 토픽 문서                        |
| Graph 보기  | 세션 간 관계 탐색                                  |
| Commands  | sync, ingest, wiki update, graph rebuild 실행 |
| Job 모니터링  | SSE 기반 진행 상태 표시                             |
| Job 취소    | 실행 중 작업 안전 중단                               |
| 태그 / 즐겨찾기 | 세션 메타데이터 편집                                 |
| 사용자 노트    | 세션별 Markdown 노트 작성                          |

REST API 엔드포인트 전체(검색 · 세션 메타 · 명령 · Job · SSE)는 **[API 레퍼런스](docs/reference/api.md)** 를 참고하세요.

---

## MCP 서버

MCP 호환 AI 에이전트(Claude Code, Cursor 등)에서 seCall 검색을 사용할 수 있습니다.

```bash
secall mcp                        # stdio 모드
secall mcp --http 127.0.0.1:8080  # HTTP 모드
```

제공 도구(`recall` / `get` / `status` / `wiki_search` / `graph_query`), MCP 클라이언트 설정, 세션 hooks 자동 동기화 예시는 **[API 레퍼런스](docs/reference/api.md)** 를 참고하세요.

---

## 볼트 구조

seCall은 Obsidian 호환 Markdown 볼트를 사용합니다.

```text
vault/
├── raw/
│   └── .sessions/
│       └── YYYY-MM-DD/
│           └── 원본 세션 Markdown
├── wiki/
│   ├── projects/
│   ├── topics/
│   └── decisions/
├── log/
│   └── YYYY-MM-DD.md
└── graph/
    └── graph.json
```

| 경로                 | 설명                                        |
| ------------------ | ----------------------------------------- |
| `raw/.sessions/`   | 불변 원본 세션. dot-prefix로 Obsidian에서 기본 숨김 처리 |
| `wiki/projects/`   | 프로젝트별 AI 생성 요약                            |
| `wiki/topics/`     | 기술 주제별 위키                                 |
| `wiki/decisions/`  | 아키텍처 의사결정 기록                              |
| `log/`             | 날짜별 작업 일기                                 |
| `graph/graph.json` | Knowledge Graph 출력                        |

원본은 Markdown 파일입니다. DB는 파생 캐시입니다.

```bash
secall reindex --from-vault
```

위 명령으로 볼트에서 DB를 다시 만들 수 있습니다.

---

## 검색 구조

seCall 검색은 세 가지 계층으로 동작합니다.

```text
query
  ├─ BM25 검색
  ├─ 벡터 검색
  └─ RRF 결합
       ↓
세션 다양성 제한
       ↓
최종 결과
```

| 구성        | 설명                                |
| --------- | --------------------------------- |
| BM25      | SQLite FTS5 기반 전문 검색              |
| 한국어 토크나이저 | Lindera ko-dic 또는 Kiwi-rs         |
| 벡터 검색     | BGE-M3 임베딩, 1024차원                |
| ANN 인덱스   | usearch HNSW                      |
| 결합 방식     | Reciprocal Rank Fusion, 기본 `k=60` |
| 다양성 제한    | 한 세션에서 최대 2개 턴만 노출                |
| 쿼리 확장     | Claude Code 기반 자연어 쿼리 확장          |

Windows에서는 일부 기능이 fallback으로 동작합니다.

| 기능           | Windows 동작                          |
| ------------ | ----------------------------------- |
| HNSW ANN 인덱스 | `usearch` 미사용, BLOB 코사인 스캔 fallback |
| Kiwi-rs      | 미지원, Lindera ko-dic fallback        |

---

## Knowledge Graph

세션과 위키에서 관계를 추출해 그래프를 만듭니다.

```bash
secall graph build
secall graph stats
secall graph export
```

시맨틱 그래프 재구축:

```bash
secall graph rebuild --retry-failed
secall graph rebuild --since 2026-04-01
secall graph rebuild --session abc12345
secall graph rebuild --all
```

옵션 우선순위:

```text
--session > --all > --retry-failed > --since
```

### 노드 타입

| 타입        | 설명                           |
| --------- | ---------------------------- |
| `session` | 개별 AI 대화 세션                  |
| `project` | 프로젝트                         |
| `agent`   | Claude Code, Codex, Gemini 등 |
| `tool`    | 세션에서 사용한 도구                  |

### 엣지 타입

| 타입                | 방식     | 설명           |
| ----------------- | ------ | ------------ |
| `belongs_to`      | 규칙 기반  | 세션이 프로젝트에 속함 |
| `by_agent`        | 규칙 기반  | 세션을 생성한 에이전트 |
| `uses_tool`       | 규칙 기반  | 사용된 도구       |
| `same_project`    | 규칙 기반  | 같은 프로젝트의 세션  |
| `same_day`        | 규칙 기반  | 같은 날짜의 세션    |
| `fixes_bug`       | LLM 기반 | 버그 수정 관계     |
| `modifies_file`   | LLM 기반 | 파일 변경 관계     |
| `introduces_tech` | LLM 기반 | 기술 도입 관계     |
| `discusses_topic` | LLM 기반 | 주제 논의 관계     |

---

## Wiki 생성

기본 위키 업데이트:

```bash
secall wiki update
```

백엔드 지정:

```bash
secall wiki update --backend claude
secall wiki update --backend codex
secall wiki update --backend haiku
```

특정 세션만 반영:

```bash
secall wiki update --session <id>
```

오프라인 / 수동 Git 모드:

```bash
secall wiki update --no-pull
```

위키 상태 확인:

```bash
secall wiki status
```

### 생성 백엔드와 리뷰 백엔드

위키 생성은 도구 호출이 가능한 백엔드에서만 동작합니다.

| 백엔드         | 생성 | 리뷰 |
| ----------- | -: | -: |
| `claude`    | 가능 | 가능 |
| `codex`     | 가능 | 가능 |
| `haiku`     | 가능 | 가능 |
| `ollama`    | 불가 | 가능 |
| `lmstudio`  | 불가 | 가능 |
| `anthropic` | 가능 | 가능 |

리뷰 백엔드 지정:

```bash
secall wiki update --review --review-backend ollama
secall config set wiki.review_backend ollama
```

---

## 작업 일기

날짜별 작업 일기를 생성합니다.

```bash
secall log
secall log 2026-04-15
```

동작 방식:

1. 해당 날짜 세션 수집
2. 프로젝트별 그룹핑
3. Knowledge Graph에서 관련 토픽 추출
4. LLM으로 산문 정리
5. `vault/log/{date}.md`에 저장

LLM이 설정되지 않은 경우 템플릿 fallback을 사용합니다.

---

## 멀티 기기 동기화

Git을 사용해 여러 기기에서 같은 볼트를 동기화할 수 있습니다.

```bash
secall sync
```

전체 sync 흐름:

```text
init
  → pull
  → reindex
  → ingest
  → wiki_update
  → graph
  → push
```

로컬 전용 모드:

```bash
secall sync --local-only
```

그래프 생략:

```bash
secall sync --no-graph
```

위키 생략:

```bash
secall sync --no-wiki
```

### 충돌 처리

| 상황                                      | 동작                                   |
| --------------------------------------- | ------------------------------------ |
| 같은 wiki 문서를 여러 기기에서 갱신                  | `sources` 합집합 기반으로 자동 재생성            |
| `raw/`, `log/`, `graph/` 등 wiki 외 파일 충돌 | 자동 중단 후 수동 해결                        |
| 오프라인 작업                                 | `--no-pull` 또는 `--local-only` 사용     |
| 같은 토픽 재생성                               | 기존 본문 누적 없이 새 본문으로 교체, `sources`만 유지 |

---

## 데이터 무결성 검사

```bash
secall lint
```

검사 규칙:

| 코드     | 설명                    |
| ------ | --------------------- |
| `L001` | DB에는 있지만 볼트 파일이 없음    |
| `L002` | 볼트에는 있지만 DB에 없는 고아 파일 |
| `L003` | FTS 인덱스 갭             |

고아 볼트 파일 자동 정리:

```bash
secall lint --fix-orphan-vault
```

---

## 설정

현재 설정 확인:

```bash
secall config show
secall config llm show
```

설정 변경:

```bash
secall config set output.timezone Asia/Seoul
secall config set search.tokenizer kiwi
secall config set embedding.backend ollama
secall config llm set log.backend haiku
```

설정 파일 경로 확인:

```bash
secall config path
```

Web UI에서 설정 편집:

```bash
secall serve --port 8080 --allow-config-edit
```

### 주요 설정 키

| 키                        | 설명                   | 기본값                       |
| ------------------------ | -------------------- | ------------------------- |
| `vault.path`             | Obsidian vault 경로    | `~/obsidian-vault/seCall` |
| `vault.git_remote`       | Git remote URL       | 없음                        |
| `vault.branch`           | Git 브랜치              | `main`                    |
| `search.tokenizer`       | `lindera` 또는 `kiwi`  | `lindera`                 |
| `search.default_limit`   | 기본 검색 결과 수           | `10`                      |
| `embedding.backend`      | 임베딩 백엔드              | `ollama`                  |
| `embedding.ollama_model` | Ollama 임베딩 모델        | `bge-m3`                  |
| `embedding.cloud_host`   | Ollama Cloud API 호스트 | `https://ollama.com`      |
| `output.timezone`        | IANA 타임존             | `UTC`                     |
| `graph.semantic_backend` | 그래프 시맨틱 추출 백엔드       | `none`                    |
| `wiki.default_backend`   | 위키 생성 백엔드            | `codex`                   |
| `wiki.review_backend`    | 위키 리뷰 백엔드            | `wiki.default_backend`    |
| `log.backend`            | 작업 일기 백엔드            | `graph.semantic_backend`  |

설정 파일 위치:

| OS      | 경로                                                 |
| ------- | -------------------------------------------------- |
| macOS   | `~/Library/Application Support/secall/config.toml` |
| Linux   | `~/.config/secall/config.toml`                     |
| Windows | `%APPDATA%\secall\config.toml`                     |

---

## CLI 레퍼런스

| 명령                                   | 설명                        |
| ------------------------------------ | ------------------------- |
| `secall init`                        | 대화형 초기화                   |
| `secall ingest [path] --auto`        | 세션 파싱 및 인덱싱               |
| `secall sync`                        | 전체 동기화                    |
| `secall recall <query>`              | 세션 검색                     |
| `secall get <id> [--full]`           | 세션 상세 조회                  |
| `secall status`                      | 인덱스 통계와 설정 요약             |
| `secall embed [--all]`               | 벡터 임베딩 생성                 |
| `secall classify [--dry-run]`        | 세션 일괄 분류                  |
| `secall lint`                        | 인덱스 / 볼트 정합성 검사           |
| `secall mcp [--http <addr>]`         | MCP 서버 시작                 |
| `secall serve [--port <port>]`       | REST API + Web UI 서버 시작   |
| `secall config show\|set\|path`      | 설정 확인 / 변경                |
| `secall graph build\|stats\|export`  | Knowledge Graph 관리        |
| `secall graph rebuild`               | 시맨틱 그래프 재구축               |
| `secall wiki update`                 | 위키 생성 / 갱신                |
| `secall wiki status`                 | 위키 상태 확인                  |
| `secall log [YYYY-MM-DD]`            | 날짜별 작업 일기 생성              |
| `secall model download\|info\|check` | ONNX 모델 관리                |
| `secall reindex --from-vault`        | 볼트에서 DB 재구축               |
| `secall migrate summary`             | summary frontmatter 일괄 추가 |

---

## 개발

개발 서버 실행:

```bash
just dev
```

`just dev`는 다음 두 서버를 함께 실행합니다.

|     포트 | 역할                                 |
| -----: | ---------------------------------- |
| `8080` | axum API 서버 + Web UI reverse proxy |
| `5173` | Vite dev server                    |

접속 방식:

| 주소                      | 설명                 |
| ----------------------- | ------------------ |
| `http://127.0.0.1:8080` | 단일 포트 진입점          |
| `http://127.0.0.1:5173` | Vite 직접 접속, HMR 사용 |

빌드:

```bash
just build
```

수동 빌드:

```bash
cd web
pnpm install
pnpm build
cd ..
cargo build --release
```

개발 요구사항:

| 도구   | 버전 / 설명    |
| ---- | ---------- |
| Rust | 1.75+      |
| Node | 22+        |
| pnpm | 9+         |
| just | 선택, 명령 통합용 |

---

## 아키텍처

```text
AI agent exports
  ├─ Claude Code JSONL
  ├─ Codex CLI JSONL
  ├─ Gemini CLI JSON
  ├─ claude.ai ZIP
  └─ ChatGPT ZIP
        ↓
ingest / normalize
        ↓
SQLite
  ├─ sessions
  ├─ turns
  ├─ FTS5
  ├─ vectors
  └─ graph metadata
        ↓
vault/
  ├─ raw/.sessions
  ├─ wiki
  ├─ log
  └─ graph
        ↓
interfaces
  ├─ CLI
  ├─ Web UI
  ├─ REST API
  ├─ Obsidian Plugin
  └─ MCP Server
```

아키텍처 이미지:

![seCall 아키텍처](arch_v0.png)

---

## 기술 스택

| 분류            | 기술                              |
| ------------- | ------------------------------- |
| 언어            | Rust 1.75+, 2021 edition        |
| 데이터베이스        | SQLite + FTS5, rusqlite bundled |
| 한국어 NLP       | Lindera ko-dic, Kiwi-rs         |
| 임베딩           | Ollama BGE-M3, ONNX Runtime     |
| ANN 인덱스       | usearch HNSW                    |
| REST API      | axum                            |
| MCP           | rmcp, stdio, Streamable HTTP    |
| Web UI        | Tailwind, shadcn/ui             |
| Obsidian 플러그인 | TypeScript, esbuild             |
| 볼트            | Obsidian 호환 Markdown            |
| 지원 플랫폼        | macOS, Windows x86_64, Linux CI |

---

## 참고한 프로젝트와 아이디어

* [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) — LLM으로 원본 자료에서 점진적 지식 베이스를 만드는 패턴
* [tobi/llm-wiki](https://github.com/tobi/llm-wiki) — LLM Wiki 구현 참고
* [qmd](https://github.com/tobi/qmd) — Markdown 파일용 로컬 검색 엔진
* [graphify](https://github.com/safishamsi/graphify) — 파일 폴더를 Knowledge Graph로 변환하는 접근

seCall은 AI 코딩 에이전트인 Claude Code와 Codex를 [tunaFlow](https://github.com/hang-in/tunaFlow) 멀티에이전트 워크플로우로 오케스트레이션하여 개발되었습니다.

---

## 업데이트 이력

전체 버전 · Phase 변경 이력은 **[CHANGELOG.md](CHANGELOG.md)** 에서 관리합니다.

---

## 라이선스

[AGPL-3.0](LICENSE)

---

<div align="center">

**Contact**: [d9ng@outlook.com](mailto:d9ng@outlook.com)

</div>
