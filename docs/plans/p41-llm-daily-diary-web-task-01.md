---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 01
title: Daily diary 다중 백엔드 (secall log --backend)
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-08
---

# Task 01 — Daily diary 다중 백엔드

## Changed files

수정:
- `crates/secall/src/commands/log.rs:7` — `pub async fn run(date: Option<String>)` 시그니처에 `backend: Option<String>` (CLI 전달분) 추가. line 123 이하의 `if config.graph.semantic_backend == "ollama"` 분기를 wiki backend 와 같은 trait dispatch 로 교체.
- `crates/secall/src/main.rs:249` 근처 (Commands::Log 정의) — `--backend <name>` flag 추가. dispatcher 가 `backend` 를 `commands::log::run` 에 전달.
- `crates/secall-core/src/vault/config.rs:18` — `Config` 에 `pub log: LogConfig` 필드 추가. 신규 `LogConfig` struct: `backend: Option<String>` (`None` = `[graph]` 폴백), `model: Option<String>`, `api_url: Option<String>`, `max_tokens: Option<u32>`. `Default` 구현.

신규:
- `crates/secall-core/src/log/mod.rs` (선택) — log 전용 backend trait 또는 wiki backend trait 재사용 (후자가 더 단순). 본 task 는 `WikiBackend` (`generate(prompt) -> String`) 재사용을 default 안으로 한다. 이 경우 신규 파일 없이 `commands/log.rs` 안에서 `secall_core::wiki::{ClaudeBackend, CodexBackend, HaikuBackend, OllamaBackend, LmStudioBackend}` 를 import 한다.

회귀 테스트:
- `crates/secall/tests/log_backend_resolve.rs` (신규) — backend resolution 우선순위 테스트 (CLI flag > `[log].backend` > `[graph].semantic_backend` > `"ollama"`). 1 test fn.

## Change description

### 1. backend resolution 우선순위

```
CLI --backend → [log].backend → [graph].semantic_backend → "ollama"
```

기존 `[graph].semantic_backend` 만 설정한 사용자도 자동으로 같은 백엔드로 일기 생성 → 회귀 없음.

### 2. WikiBackend trait 재사용

log 는 본질적으로 "프롬프트 → 텍스트 응답" 이라 wiki backend 와 같은 인터페이스. 별도 LogBackend trait 도입 X. `commands/log.rs` 에서 backend_name 매칭 → 적절한 `Box<dyn WikiBackend>` 생성. `WikiBackend::generate(prompt)` 호출.

backend 5종 분기:
- `claude` → `ClaudeBackend { model, vault_path }` (vault_path 는 config 에서)
- `codex` → `CodexBackend { model, vault_path }`
- `haiku` → `HaikuBackend::from_env(model, max_tokens, system_prompt)` (`ANTHROPIC_API_KEY` 필요)
- `ollama` → `OllamaBackend { api_url, model, max_tokens }`
- `lmstudio` → `LmStudioBackend { api_url, model, max_tokens }`
- 그 외 → `anyhow::bail!("Unknown log backend ...")`

### 3. CLI flag

```
secall log [date] [--backend claude|codex|haiku|ollama|lmstudio] [--model <name>]
```

`--model` 도 추가 (backend 의 model override). 둘 다 `Option<String>`. main.rs 의 `Commands::Log` enum variant 갱신.

### 4. `[log]` config 섹션

```toml
[log]
backend = "ollama"          # default 폴백 (graph.semantic_backend → "ollama")
model = "gemma4:e4b"         # backend default 와 동일하면 None
api_url = "http://localhost:11434"
max_tokens = 4096
```

### 5. 회귀 테스트

`tests/log_backend_resolve.rs`:
- CLI flag 만: `("haiku", None, None) → "haiku"`
- `[log].backend = "claude"`: `(None, Some("claude"), None) → "claude"`
- `[graph].semantic_backend = "gemini"` (legacy): `(None, None, Some("gemini")) → "gemini"`
- 모두 미설정: `(None, None, None) → "ollama"`

(실제 LLM 호출은 mock 또는 skip — resolution 함수만 단위 테스트.)

## Dependencies

- 없음 (parallel_group A, depends_on 없음). task 02 가 같은 config.rs 영역 손대니 02 먼저 머지 권장.
- crate dep: 추가 없음. 기존 `secall_core::wiki` 의 backend impl 들 재사용.

## Verification

```bash
# 1. cargo check + clippy
cargo check -p secall-core
cargo check -p secall
cargo clippy --all-targets --all-features

# 2. 회귀 테스트
cargo test -p secall --test log_backend_resolve

# 3. CLI help — flag 등록 확인
./target/debug/secall log --help | grep -E "backend|model"

# 4. (수동) backend 별 동작 확인 (외부 인증 필요)
secall log 2026-05-08 --backend ollama --model gemma4:e4b
secall log 2026-05-08 --backend haiku   # ANTHROPIC_API_KEY 필요
secall log 2026-05-08 --backend claude  # claude code CLI 필요
```

## Risks

- **WikiBackend 의 vault_path 의존** — `ClaudeBackend` / `CodexBackend` 는 vault_path 가 prompt 안의 link 정규화 등에 쓰임. log 에서 그게 의미 있는지 검증 필요. 의미 없으면 dummy path 전달 또는 별도 어댑터.
- **HaikuBackend 의 system_prompt** — wiki 용 system prompt 가 일기 톤과 어긋날 수 있음. log 전용 system_prompt 를 `docs/prompts/log-system.md` 에 두고 backend 생성 시 inject (또는 user prompt 안에 instruction 포함).
- **gemini-cli backend** — wiki 에 gemini CLI backend 가 없으므로 본 task 도 gemini-cli 는 제외. gemini API (web) 는 graph 에만 있고 wiki 에는 없음 → log 도 동일. 본 plan 의 non-goal 에 명시.
- **외부 CLI 미설치 에러** — `claude` / `codex` 가 PATH 에 없을 때 친절한 에러 메시지 (현재 wiki backend 가 어떻게 처리하는지 따름).

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — backend 본체. import 만 하고 변경 X.
- `crates/secall-core/src/graph/semantic.rs` — task 02 영역.
- `crates/secall-core/src/search/embedding.rs` — embedding 영역.
- `crates/secall/src/commands/wiki.rs` — wiki 명령. log 와 코드 일부 비슷해도 본 task 는 log.rs 만 손댐.
