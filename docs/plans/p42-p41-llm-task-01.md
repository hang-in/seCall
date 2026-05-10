---
type: task
plan_slug: p42-p41-llm
task_id: 01
title: log.rs 하드코딩 default 정리
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 01 — log.rs 하드코딩 default 정리

## Changed files

수정:
- `crates/secall/src/commands/log.rs:1-7` — 상단 `use` 블록의
  `secall_core::llm::defaults::{...}` 임포트에
  `WIKI_CLAUDE_DEFAULT`, `WIKI_CODEX_DEFAULT`, `GRAPH_LMSTUDIO_DEFAULT` 추가.
- `crates/secall/src/commands/log.rs:236` — `"sonnet".to_string()` →
  `WIKI_CLAUDE_DEFAULT.to_string()`. 함께 `warn_using_default("log.model[claude]", WIKI_CLAUDE_DEFAULT)`
  호출 (config / CLI 둘 다 미설정 시에만 trip — 현재 구조상 `resolved_model.unwrap_or_else` 의 closure 안).
- `crates/secall/src/commands/log.rs:246` — `"gpt-5.4".to_string()` →
  `WIKI_CODEX_DEFAULT.to_string()` + 동일 warn.
- `crates/secall/src/commands/log.rs:280` — `"gemma-4-e4b-it".to_string()` →
  `GRAPH_LMSTUDIO_DEFAULT.to_string()` + 동일 warn.

회귀 테스트:
- `crates/secall/tests/log_backend_resolve.rs` 갱신 — 기존 1 test fn 유지하고
  새 test fn `model_resolution_priority_matches_plan` 추가:
  CLI `--model` > `[log].model` > backend default 순으로 resolve 되는지 검증.
  `resolve_log_model` 이 `pub` 가 아니므로 본 task 에서 `pub fn resolve_log_model(...)`
  로 가시성 변경 (현재 `fn` private). 단, 다른 호출자는 없으므로 영향 없음.

## Change description

### 1. import 추가

```rust
use secall_core::{
    llm::defaults::{
        warn_using_default,
        GRAPH_LMSTUDIO_DEFAULT,
        LOG_GEMINI_DEFAULT,
        LOG_OLLAMA_DEFAULT,
        WIKI_CLAUDE_DEFAULT,
        WIKI_CODEX_DEFAULT,
    },
    ...
};
```

### 2. 3개 fallback 상수화

`generate_log_body` 의 backend match arm:

- claude (line ~236):
  ```rust
  let model = resolved_model.unwrap_or_else(|| {
      warn_using_default("log.model[claude]", WIKI_CLAUDE_DEFAULT);
      WIKI_CLAUDE_DEFAULT.to_string()
  });
  ```
- codex (line ~246): 동일 패턴, `WIKI_CODEX_DEFAULT`.
- lmstudio (line ~280): 동일 패턴, `GRAPH_LMSTUDIO_DEFAULT`.

ollama / gemini 는 이미 상수 사용 중 — 변경 없음.

### 3. `resolve_log_model` 의 가시성

테스트에서 model resolution 순서를 직접 검증하려면 `pub fn` 필요.
대안: `resolve_log_model` 를 그대로 두고 새 `pub fn resolve_log_model_for_test`
같은 wrapper 도입. 본 task 는 단순히 `pub fn resolve_log_model` 로 변경 (외부 호출자 0건).

### 4. 회귀 테스트

`tests/log_backend_resolve.rs`:

```rust
#[test]
fn model_resolution_priority_matches_plan() {
    let mut config = Config::default();

    // CLI > config > backend default
    config.log.model = Some("config-model".into());
    assert_eq!(
        resolve_log_model(&config, "ollama", Some("cli-model")),
        Some("cli-model".into()),
    );
    assert_eq!(
        resolve_log_model(&config, "ollama", None),
        Some("config-model".into()),
    );

    // 미설정 시 backend default
    config.log.model = None;
    assert_eq!(
        resolve_log_model(&config, "ollama", None).as_deref(),
        Some(LOG_OLLAMA_DEFAULT),
    );
    assert_eq!(
        resolve_log_model(&config, "gemini", None).as_deref(),
        Some(LOG_GEMINI_DEFAULT),
    );

    // claude / codex / lmstudio 는 None 반환 (generate_log_body 의 match arm 책임)
    assert_eq!(resolve_log_model(&config, "claude", None), None);
    assert_eq!(resolve_log_model(&config, "codex", None), None);
    assert_eq!(resolve_log_model(&config, "lmstudio", None), None);
}
```

## Dependencies

- 의존 task 없음. `defaults.rs` 의 상수는 P41 task 02 에서 이미 export.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall
cargo clippy --all-targets -p secall

# 2. 신규 + 기존 테스트
cargo test -p secall --test log_backend_resolve

# 3. grep — 하드코딩 잔존 0 확인
grep -nE '"sonnet"|"gpt-5\.4"|"gemma-4-e4b-it"' crates/secall/src/commands/log.rs
# 출력이 없어야 함 (exit 1 = 매칭 0건 = OK)

# 4. (수동) help 영향 없음 — Commands::Log 시그니처 변경 X
./target/debug/secall log --help | grep -E "backend|model"
```

## Risks

- **`warn_using_default` 의 noise** — `OnceLock` 으로 1회만 trip 하므로 동일 process 안에서 spam 없음.
  여러 day 의 log 를 batch 로 만드는 케이스에서도 process 1개 = warn 1회.
- **`resolve_log_model` 의 가시성 확장** — `pub fn` 으로 바꾸면 외부 crate 에서도 호출 가능.
  현재 호출자 0건이지만 추후 의도치 않은 사용 가능. mitigation: doc comment 에 "internal only — for test access" 명시.
- **claude / codex 의 backend 측 default** — `WIKI_CLAUDE_DEFAULT = "sonnet"` 은 현재 backend impl 의 기대값.
  backend 측 default 가 바뀌면 P42 task 01 의 회귀 테스트가 trip → 의도 확인 가능.

## Scope boundary (수정 금지)

- `crates/secall-core/src/llm/defaults.rs` — 상수 자체 변경 X (task 02 도 동일).
- `crates/secall-core/src/graph/semantic.rs` — task 02 영역.
- `crates/secall/src/commands/config.rs` — task 03 영역.
- `crates/secall/src/main.rs` — `Commands::Log` 시그니처 변경 X.
- `crates/secall-core/src/wiki/{claude,codex,lmstudio}.rs` — backend 본체 변경 X.
- `web/` — task 04 영역.
