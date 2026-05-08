---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 05
title: CLI `secall config` 강화
parallel_group: A
depends_on: [02]
status: pending
updated_at: 2026-05-08
---

# Task 05 — CLI `secall config` 강화

## Changed files

수정:
- `crates/secall/src/commands/config.rs:6` (`run_show`) — 현재 단순 dump 를 sanitized + 카테고리 별 헤더 + 환경변수 indicator 출력으로 강화.
- `crates/secall/src/commands/config.rs:55` (`run_set`) — 기존 generic key=value set 에 LLM 섹션 alias 추가 (예: `secall config set log.backend haiku` 가 `[log].backend` 갱신).
- `crates/secall/src/commands/config.rs:143` (`run_path`) — 단순 출력 + `--copy` 옵션 (path 를 클립보드에 복사 — macOS 의 `pbcopy` 기반, optional).
- `crates/secall/src/main.rs:256` (`ConfigAction` enum) — 신규 서브커맨드: `Llm { action: LlmAction }`.

신규:
- `crates/secall-core/src/llm/summary.rs` (선택) — config 의 LLM 섹션 sanitized summary 를 생성하는 helper. `run_show` + REST `do_config_get` 양쪽 재사용.

회귀 테스트:
- `crates/secall/tests/config_llm_cli.rs` (신규) — `secall config llm show` / `secall config llm set log.backend haiku` 의 stdout/exit code 회귀. 1-2 test fn (subprocess 또는 CLI 직접 호출).

## Change description

### 1. `secall config show` 강화

현재는 raw config 출력 (toml dump 추정). 강화 후:

```
$ secall config show

Vault
  path: /Users/d9ng/obsidian-vault/seCall
  branch: main

Wiki
  default_backend: lmstudio
  review_model: sonnet
  backends:
    haiku: model=claude-haiku-4-5-20251001 max_tokens=4096
    ollama: api_url=http://localhost:11434 model=gemma3:27b
    ...

Graph
  semantic: true
  semantic_backend: gemini
  gemini_model: gemini-2.5-flash
  gemini_api_key: <env: SECALL_GEMINI_API_KEY = set>

Log
  backend: ollama
  model: gemma4:e4b

Embedding
  backend: ollama
  ollama_url: http://localhost:11434
  ollama_model: bge-m3

Environment indicators
  ANTHROPIC_API_KEY: set
  SECALL_GEMINI_API_KEY: set
  OPENAI_API_KEY: not set
```

### 2. `secall config set <key> <value>` 동작 유지 + LLM 별칭

현재 `run_set` 이 dot-path 기반이면 그대로 활용. 추가로 자주 쓰는 키의 별칭:

```
secall config set log.backend haiku
secall config set wiki.default_backend ollama
secall config set graph.gemini_model gemini-2.5-pro
secall config set embedding.ollama_model bge-m3
```

### 3. `secall config llm` 서브커맨드 (신규)

```
secall config llm show       # LLM 섹션만 sanitized summary
secall config llm where      # config.toml 경로 + 어떤 키가 어떤 카테고리에 영향 미치는지
secall config llm test       # 각 백엔드에 짧은 ping (예: "1+1?" 같은 1-token 응답) — 인증/연결 검증
```

`llm test` 는 본 task 의 stretch goal. 시간 부족하면 다음 plan 으로 미룸.

### 4. `secall config path --copy`

```
secall config path           # 단순 출력
secall config path --copy    # macOS: pbcopy, Linux: xclip / xsel (없으면 stderr 에 안내)
```

### 5. 회귀 테스트

`tests/config_llm_cli.rs`:
- `secall config show` 출력에 "Wiki" / "Graph" / "Log" / "Embedding" 헤더 포함.
- 환경변수 indicator 가 set/not-set 상태에 맞게 표시.
- `secall config set log.backend haiku` 후 toml 파일에 반영.

## Dependencies

- **task 02 필수** — `[log]` 섹션 + LLM constants. 본 task 의 sanitize 대상.
- crate dep: 추가 없음.

## Verification

```bash
cargo check -p secall
cargo test -p secall --test config_llm_cli

# (수동) CLI 동작
./target/debug/secall config show
./target/debug/secall config llm show
./target/debug/secall config set log.backend haiku
./target/debug/secall config show | grep -A1 Log

# (수동) path --copy
./target/debug/secall config path --copy
pbpaste   # macOS — 복사된 path 확인
```

## Risks

- **toml round-trip 시 주석 손실** — `run_set` 이 toml 직렬화 사용하면 task 03 와 동일 한계. 별도 `toml_edit` 도입은 후속 plan.
- **`llm test` 의 외부 의존** — claude / codex CLI 미설치 환경에서 실패. friendly error.
- **Sanitize 의 누락** — env-only secret 외에도 향후 추가될 secret 필드를 모듈로 넘겨야 함. `llm/summary.rs` 가 단일 진실 출처가 되도록.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/` — backend 본체.
- `crates/secall/src/commands/wiki.rs`, `commands/log.rs` — task 01 / 02 영역.
- `crates/secall-core/src/mcp/` — REST 영역 (task 03).
- `web/` — task 04 영역.
