---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 01
title: WikiReviewer trait 도입
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 01 — WikiReviewer trait 도입

## Changed files

수정:
- `crates/secall-core/src/wiki/review.rs:19-105` — 기존 `pub async fn review_page(...)` 의 anthropic API 호출 본체를 `AnthropicReviewer` struct + `WikiReviewer` impl 로 이동. 함수 시그니처는 유지하되 내부에서 `AnthropicReviewer { api_key, model }.review(...)` 를 호출하는 wrapper 로 단순화 (downstream 영향 0).
- `crates/secall-core/src/wiki/mod.rs` — `pub use review::{ReviewResult, ReviewIssue, WikiReviewer, AnthropicReviewer};` 추가. 외부 crate (secall) 가 trait 객체를 빌드할 수 있도록 re-export.

신규:
- 신규 파일 없음. trait + impl 모두 `review.rs` 안에 추가 (300줄 이내 유지).

회귀 테스트:
- `crates/secall-core/src/wiki/review.rs` 의 `#[cfg(test)] mod tests` 안에 1 test fn `anthropic_reviewer_implements_wiki_reviewer` — type assertion (`fn assert_impl<T: WikiReviewer>() {}; assert_impl::<AnthropicReviewer>();`) + ReviewResult 의 default 값 (issues empty, approved false) 검증. 외부 호출 X — pure compile-time + struct 검증.

## Change description

### 1. trait 정의

```rust
#[async_trait::async_trait]
pub trait WikiReviewer: Send + Sync {
    async fn review(
        &self,
        page_content: &str,
        source_summary: &str,
    ) -> anyhow::Result<ReviewResult>;
}
```

`async_trait` crate 가 워크스페이스 dep 에 이미 있음 (다른 backend trait 에서 사용 — `wiki/mod.rs` 의 `WikiBackend` 가 같은 패턴). 신규 dep 0건.

### 2. AnthropicReviewer struct + impl

```rust
pub struct AnthropicReviewer {
    pub api_key: String,
    pub model: String,
}

#[async_trait::async_trait]
impl WikiReviewer for AnthropicReviewer {
    async fn review(
        &self,
        page_content: &str,
        source_summary: &str,
    ) -> anyhow::Result<ReviewResult> {
        // 기존 review.rs:19-105 의 본체 그대로 이동
        // model_id 매칭 ("opus" / "sonnet") + payload + POST + JSON parse
    }
}
```

### 3. 기존 `review_page` wrapper 보존

```rust
/// Backwards-compat wrapper. 기존 호출자 (commands/wiki.rs:350 등) 가
/// 그대로 동작하도록 유지. P43 task 03 가 호출자 측에서 trait 객체로 전환.
pub async fn review_page(
    api_key: &str,
    model: &str,
    page_content: &str,
    source_summary: &str,
) -> anyhow::Result<ReviewResult> {
    AnthropicReviewer {
        api_key: api_key.to_string(),
        model: model.to_string(),
    }
    .review(page_content, source_summary)
    .await
}
```

이 단계에서는 동작 변경 없음 — task 02 가 다른 reviewer 추가, task 03 이 dispatcher 구현.

### 4. mod.rs re-export

```rust
// crates/secall-core/src/wiki/mod.rs
pub use review::{AnthropicReviewer, ReviewIssue, ReviewResult, WikiReviewer};
```

기존 export 가 있으면 유지 + 신규만 추가.

### 5. compile-time 회귀 테스트

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn assert_impl<T: WikiReviewer>() {}

    #[test]
    fn anthropic_reviewer_implements_wiki_reviewer() {
        assert_impl::<AnthropicReviewer>();
    }

    #[test]
    fn review_result_defaults_to_unapproved_no_issues() {
        let r: ReviewResult = serde_json::from_str("{}").unwrap();
        assert!(!r.approved);
        assert!(r.issues.is_empty());
    }
}
```

## Dependencies

- 의존 task 없음. 본 task 가 다른 task 의 prerequisite.
- crate dep: `async_trait` (워크스페이스 dep 이미 있음 — `crates/secall-core/Cargo.toml` 의 `WikiBackend` 사용 위치 grep 으로 확인 후 진행).

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. trait + struct 회귀 테스트
cargo test -p secall-core --lib wiki::review::tests

# 3. 기존 review_page wrapper 호환성 — wiki crate 가 컴파일되는지
cargo check -p secall

# 4. 기존 anthropic 동작 영향 0 (수동, ANTHROPIC_API_KEY 있는 환경)
# secall wiki update <session> --review
# (실제 호출은 task 03 이후로 미루고 본 task 는 컴파일/타입만 검증)
```

## Risks

- **trait 객체 size** — `Box<dyn WikiReviewer>` 는 fat pointer. 본 task 는 trait object 사용 자체는 미루고 task 03 에서 dispatcher 도입. 본 task 는 trait 정의 + 한 impl 만.
- **`async_trait` macro 의 ergonomic** — `Send` bound 가 필요하면 `#[async_trait::async_trait]` 의 default 가 `Send` 요구. 본 task 는 그대로 따름.
- **mod.rs re-export 충돌** — 기존 `pub use` 와 이름 충돌 검사. `ReviewResult`, `ReviewIssue` 가 이미 export 됐다면 신규 (`WikiReviewer`, `AnthropicReviewer`) 만 추가.
- **`review_page` 시그니처 유지** — task 03 가 dispatcher 만들 때 wrapper 를 deprecated 처리. 본 task 에서는 deprecation X — 호환 보장만.
- **`include_str!` 경로** — `review.rs:118` 의 `include_str!("../../../../docs/prompts/wiki-review.md")` 는 본 task 에서 변경 X (task 04 영역).

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — task 02 영역.
- `crates/secall/src/commands/wiki.rs` — task 03 영역 (호출자 측 dispatcher).
- `crates/secall-core/src/vault/config.rs` — task 03 / 05 영역.
- `crates/secall-core/src/wiki/mod.rs` — re-export 외 변경 X.
- `docs/prompts/wiki-review.md` — task 04 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 06 영역.
