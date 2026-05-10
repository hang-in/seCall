---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 03
title: config + CLI 통합 (review_backend dispatcher)
parallel_group: C
depends_on: [01, 02]
status: pending
updated_at: 2026-05-09
---

# Task 03 — config + CLI 통합

## Changed files

수정:
- `crates/secall-core/src/vault/config.rs` (`WikiConfig` 정의 영역) — `pub review_backend: Option<String>` 필드 추가. doc comment 에 "review backend (None 이면 default_backend → 'haiku')".
- `crates/secall/src/main.rs` (Commands::Wiki / WikiAction enum) — `--review-backend <name>` flag 추가. dispatcher 가 `commands::wiki::run_*` 에 전달.
- `crates/secall/src/commands/wiki.rs:917-925` 의 `resolve_review_model` 옆에 `resolve_review_backend(cli, config) -> String` 추가. 우선순위: CLI > `[wiki].review_backend` > `[wiki].default_backend` > "haiku".
- `crates/secall/src/commands/wiki.rs:1028` 의 `async fn run_review(model, page, summary)` 시그니처를 `async fn run_review(backend: &dyn WikiReviewer, page, summary)` 로 교체. 호출자 4곳 (line 350, 390, 481, 518) 갱신.
- `crates/secall/src/commands/wiki.rs:88,140,162` 의 review_model 전달 chain 옆에 review_backend chain 추가.
- `crates/secall/src/commands/config.rs::run_set` — `wiki.review_backend` key 추가 (set 가능).

신규:
- `crates/secall/src/commands/wiki.rs` 안에 `fn build_reviewer(config: &Config, backend_name: &str, model: &str) -> Result<Box<dyn WikiReviewer>>` helper. backend 이름 매칭 → 적절한 reviewer struct + Box. 외부 secret/url 은 config 에서.

회귀 테스트:
- `crates/secall/tests/wiki_review_resolve.rs` (신규) — `resolve_review_backend` 의 우선순위 검증 (CLI > config.review_backend > default_backend > "haiku"). 1 test fn, 4 case.

## Change description

### 1. config schema

```rust
// crates/secall-core/src/vault/config.rs
pub struct WikiConfig {
    // ... existing fields ...
    /// Review backend name. `None` 이면 `default_backend` → "haiku" 폴백.
    /// Valid: claude / codex / haiku / ollama / lmstudio.
    pub review_backend: Option<String>,
    // review_model 은 그대로 유지 (P41 task 02 의 WIKI_REVIEW_DEFAULT 와 호환)
}
```

### 2. CLI flag

```
secall wiki update [session] [--review] [--review-backend <name>] [--review-model <name>]
```

기존 `--review-model` 유지 + `--review-backend` 신규. 둘 다 `Option<String>`.

### 3. dispatcher

```rust
// commands/wiki.rs
fn build_reviewer(
    config: &Config,
    backend_name: &str,
    model: &str,
) -> Result<Box<dyn WikiReviewer>> {
    use secall_core::wiki::{
        AnthropicReviewer, ClaudeReviewer, CodexReviewer,
        HaikuReviewer, LmStudioReviewer, OllamaReviewer,
    };

    match backend_name {
        "anthropic" | "sonnet" | "opus" => {
            // legacy — review_model 직접 사용 (P41 sonnet/opus 호환)
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            Ok(Box::new(AnthropicReviewer { api_key, model: model.to_string() }))
        }
        "claude" => Ok(Box::new(ClaudeReviewer {
            model: model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        "codex" => Ok(Box::new(CodexReviewer {
            model: model.to_string(),
            vault_path: config.vault.path.clone(),
        })),
        "haiku" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY not set"))?;
            Ok(Box::new(HaikuReviewer {
                api_key,
                model: model.to_string(),
                max_tokens: 2048,
            }))
        }
        "ollama" => Ok(Box::new(OllamaReviewer {
            api_url: config.graph.ollama_url.clone()
                .unwrap_or_else(|| "http://localhost:11434".into()),
            model: model.to_string(),
        })),
        "lmstudio" => Ok(Box::new(LmStudioReviewer {
            api_url: config.graph.ollama_url.clone()
                .unwrap_or_else(|| "http://localhost:1234".into()),
            model: model.to_string(),
        })),
        other => anyhow::bail!("unknown review backend: {other}"),
    }
}
```

### 4. resolve_review_backend

```rust
fn resolve_review_backend(cli: Option<&str>, config: &Config) -> String {
    cli.map(ToOwned::to_owned)
        .or_else(|| config.wiki.review_backend.clone())
        .unwrap_or_else(|| {
            // default_backend 가 5종 reviewer 와 호환되면 그대로,
            // 아니면 "haiku" (anthropic-compatible) 폴백
            let db = &config.wiki.default_backend;
            if matches!(db.as_str(), "claude" | "codex" | "haiku" | "ollama" | "lmstudio") {
                db.clone()
            } else {
                "haiku".to_string()
            }
        })
}
```

### 5. run_review 시그니처 변경

기존:
```rust
async fn run_review(model: &str, page_content: &str, source_summary: &str) -> bool;
```

신규:
```rust
async fn run_review(reviewer: &dyn WikiReviewer, page_content: &str, source_summary: &str) -> bool;
```

호출자 (line 350, 390, 481, 518) 는 `run_review(&*reviewer_box, ...)` 로 호출.

`reviewer_box` 는 `generate_wiki_page` 진입 시 `build_reviewer(config, backend_name, model)?` 로 1회 빌드.

### 6. config set 별칭

```bash
secall config set wiki.review_backend ollama
```

`commands/config.rs::run_set` 의 match arm 에 `"wiki.review_backend" => config.wiki.review_backend = Some(value.to_string())` 추가.

### 7. 회귀 테스트

`tests/wiki_review_resolve.rs`:

```rust
use secall::commands::wiki::resolve_review_backend;
use secall_core::vault::Config;

#[test]
fn review_backend_priority() {
    let mut config = Config::default();
    config.wiki.default_backend = "ollama".into();
    config.wiki.review_backend = Some("claude".into());

    // CLI > config.review_backend > default_backend > "haiku"
    assert_eq!(resolve_review_backend(Some("haiku"), &config), "haiku");
    assert_eq!(resolve_review_backend(None, &config), "claude");

    config.wiki.review_backend = None;
    assert_eq!(resolve_review_backend(None, &config), "ollama");

    config.wiki.default_backend = "non-existent-backend".into();
    assert_eq!(resolve_review_backend(None, &config), "haiku");
}
```

`resolve_review_backend` 가 `pub` 이어야 외부 test 에서 import 가능 — 본 task 에서 가시성 변경.

## Dependencies

- task 01 (WikiReviewer trait), task 02 (5 reviewer impl) 둘 다 필수.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo check -p secall
cargo clippy --all-targets

# 2. 회귀 테스트
cargo test -p secall --test wiki_review_resolve

# 3. CLI help — flag 등록 확인
./target/debug/secall wiki update --help | grep -E "review-backend|review-model"

# 4. config set 별칭
./target/debug/secall config set wiki.review_backend ollama
./target/debug/secall config show | grep -A2 Wiki | grep review_backend

# 5. (수동) ollama 로 review 실행 (외부 의존)
ollama serve &
secall wiki update <session> --review --review-backend ollama
```

## Risks

- **`run_review` 시그니처 변경의 호출자 4곳** — line 350, 390, 481, 518 모두 갱신 필요. grep 으로 누락 검사: `grep -n "run_review(" crates/secall/src/commands/wiki.rs`.
- **`Box<dyn WikiReviewer>` 의 lifetime** — `&*reviewer_box` 로 deref 사용 OK. 또는 `reviewer_box.as_ref()` 의 `&dyn`.
- **legacy `--review-model` 의 의미** — 기존 사용자가 `--review-model opus` 로 사용 중. 본 task 의 dispatcher 가 `--review-model` 의 model name 을 reviewer 의 `model` 필드로 전달. backend 가 `anthropic`/`sonnet`/`opus` 면 `AnthropicReviewer` 의 model 매칭 ("opus" → claude-opus-4-6) 기존 로직 유지.
- **default_backend 가 5 reviewer 와 호환 안 됨** — 예: `default_backend = "gemini"` 면 review backend fallback 이 "haiku". 사용자가 `--review-backend` 명시 권장. doc 에 명시 (task 07).
- **`build_reviewer` 의 secret 노출** — ANTHROPIC_API_KEY 가 안 잡힐 때 friendly error. config 의 `gemini_api_key` 같은 secret 은 reviewer 에서 사용 X (gemini reviewer 본 plan 영역 외).

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/review.rs` — task 01 영역.
- `crates/secall-core/src/wiki/reviewers/*.rs` — task 02 영역.
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — backend impl 본체.
- `crates/secall-core/src/vault/config.rs` 의 `Config::save` — task 05 영역.
- `docs/prompts/wiki-review.md` — task 04 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 06 영역.
- `web/` — 본 plan 의 non-goal.
