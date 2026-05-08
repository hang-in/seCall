---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 02
title: 하드코딩된 모델 default config 노출
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-08
---

# Task 02 — 하드코딩된 모델 default config 노출

## Changed files

수정:
- `crates/secall-core/src/graph/semantic.rs:284` — `cfg.gemini_model.as_deref().unwrap_or("gemini-2.5-flash")` → fallback 시 `tracing::warn!` 추가.
- `crates/secall-core/src/graph/semantic.rs:419` — `config.ollama_model.as_deref().unwrap_or("gemma4:e4b")` 동일 처리.
- `crates/secall-core/src/graph/semantic.rs:435` — `config.ollama_model.as_deref().unwrap_or("gemma-4-e4b-it")` (lmstudio 분기) 동일.
- `crates/secall/src/commands/wiki.rs:561-565` — `resolve_backend_model` 의 `"sonnet" / "gpt-5.4"` fallback 에 `tracing::warn!` + 더 명시적인 default 안내.
- `crates/secall/src/commands/log.rs:129` — task 01 가 분기 자체를 바꾸지만 잔존 fallback 도 task 02 가 한 번에 정리.
- `crates/secall-core/src/vault/config.rs:155-168` — `GraphConfig` 의 `ollama_model` / `anthropic_model` / `gemini_model` 의 doc comment 에 default 명시. 새로운 `default_*` 함수 도입 (constants 일원화).

신규:
- `crates/secall-core/src/llm/defaults.rs` (신규) — 모든 hard-coded default 를 모은 constants 모듈. 예: `pub const GRAPH_GEMINI_DEFAULT: &str = "gemini-2.5-flash";`. 단일 진실 출처.

회귀 테스트:
- `crates/secall-core/tests/llm_defaults.rs` (신규) — defaults 모듈의 const 가 기대값과 일치하는지 (회귀 방지). 1-2 test fn.

## Change description

### 1. constants 일원화 (`llm/defaults.rs`)

코드 곳곳의 hard-coded 모델 이름을 모듈 한 곳에 모은다:

```
GRAPH_OLLAMA_DEFAULT   = "gemma4:e4b"
GRAPH_LMSTUDIO_DEFAULT = "gemma-4-e4b-it"
GRAPH_GEMINI_DEFAULT   = "gemini-2.5-flash"
GRAPH_ANTHROPIC_DEFAULT = "claude-haiku-4-5-20251001"
WIKI_CLAUDE_DEFAULT    = "sonnet"
WIKI_CODEX_DEFAULT     = "gpt-5.4"
LOG_OLLAMA_DEFAULT     = GRAPH_OLLAMA_DEFAULT  // 재사용
```

`secall-core/src/lib.rs` 에 `pub mod llm;` 등록.

### 2. 모든 fallback 에 `tracing::warn!`

config 미설정 시 어떤 default 가 적용됐는지 사용자에게 보여주기 위해:

```
config 의 graph.gemini_model 미설정 → "gemini-2.5-flash" 사용 (config 에 명시하면 이 경고 사라집니다)
```

`tracing::warn!` 의 target 은 `secall::llm_defaults` 같이 분리해서 사용자가 logfilter 로 끌 수 있게.

### 3. config 의 doc comment 갱신

`GraphConfig::ollama_model` 의 doc 에 "기본: gemma4:e4b" 같이 명시. `WikiBackendConfig::model` 도 backend 별 default 를 doc 에 적음.

### 4. 회귀 테스트

```rust
#[test]
fn test_llm_defaults_unchanged() {
    use secall_core::llm::defaults::*;
    assert_eq!(GRAPH_OLLAMA_DEFAULT, "gemma4:e4b");
    assert_eq!(GRAPH_GEMINI_DEFAULT, "gemini-2.5-flash");
    // ...
}
```

향후 default 변경 시 의도적인지 확인용.

## Dependencies

- 없음 (parallel_group A). task 01 과 같은 config.rs 영역 손대므로 task 02 먼저 머지 권장.
- crate dep: `tracing` (이미 워크스페이스 dep).

## Verification

```bash
cargo check -p secall-core
cargo test -p secall-core --test llm_defaults
cargo test -p secall-core --lib graph::
cargo test -p secall --lib commands::

# (수동) config 미설정 + 실행 시 warn 로그 확인
RUST_LOG=warn secall graph rebuild --since 2026-05-01 2>&1 | grep "config 의"
```

## Risks

- **CLI default 와 config default 의 충돌** — 예: `secall wiki update` 의 `--model` flag 가 config 무시. 현재 패턴 (CLI > config > default) 이라 문제 없으나, doc 에 우선순위 명시 필요 (task 06).
- **default 값 변경의 광범위한 영향** — 예: `gemini-2.5-flash` 가 Google 에서 deprecation 되면 default 변경해야 함. constants 한 곳에 모은 게 그 변경을 단순화하는 게 본 task 의 부수 효과.
- **tracing::warn! 의 noise** — 매 호출마다 warn 면 log 가 시끄러움. fix: process 시작 시 한 번 (lazy_static 또는 `OnceLock`).

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — backend 본체.
- `crates/secall-core/src/search/embedding.rs` — embedding 모델 default 는 별도 (P40 의 `bge-m3`). 본 task 영역 외.
- `crates/secall/src/commands/log.rs` 의 분기 변경 자체는 task 01 영역 — task 02 는 fallback 의 warn + 모듈화만 손댐.
