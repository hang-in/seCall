---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 02
title: 5 backend reviewer 구현 (claude / codex / haiku / ollama / lmstudio)
parallel_group: B
depends_on: [01]
status: pending
updated_at: 2026-05-09
---

# Task 02 — 5 backend reviewer 구현

## Changed files

신규:
- `crates/secall-core/src/wiki/reviewers/mod.rs` (신규) — 5 backend reviewer 모듈 + `pub use` re-export.
- `crates/secall-core/src/wiki/reviewers/claude.rs` (신규) — `ClaudeReviewer` impl (CLI subprocess).
- `crates/secall-core/src/wiki/reviewers/codex.rs` (신규) — `CodexReviewer` impl (CLI subprocess).
- `crates/secall-core/src/wiki/reviewers/haiku.rs` (신규) — `HaikuReviewer` impl (Anthropic API, sonnet/opus 외 모델).
- `crates/secall-core/src/wiki/reviewers/ollama.rs` (신규) — `OllamaReviewer` impl (HTTP `/api/chat` + `format: "json"`).
- `crates/secall-core/src/wiki/reviewers/lmstudio.rs` (신규) — `LmStudioReviewer` impl (HTTP `/v1/chat/completions` + `response_format: { type: "json_object" }`).

수정:
- `crates/secall-core/src/wiki/mod.rs` — `pub mod reviewers;` + `pub use reviewers::{ClaudeReviewer, CodexReviewer, HaikuReviewer, OllamaReviewer, LmStudioReviewer};` 추가.

회귀 테스트:
- `crates/secall-core/tests/wiki_reviewers.rs` (신규) — `OllamaReviewer` / `LmStudioReviewer` 의 mockito mock 테스트 (JSON 정상 응답 + parse 실패 → retry → 성공). 4 test fn.

## Change description

### 1. 공통 helper

`reviewers/mod.rs`:

```rust
pub(crate) fn build_user_prompt(page_content: &str, source_summary: &str) -> String {
    format!(
        "## 위키 페이지 내용\n\n{}\n\n## 원본 세션 요약\n\n{}",
        page_content, source_summary
    )
}

pub(crate) fn parse_review_response(raw: &str) -> anyhow::Result<crate::wiki::ReviewResult> {
    // 1) JSON object 추출 (```json fence 제거 + 첫 { ... } 매칭)
    // 2) serde_json::from_str → ReviewResult
    // 3) 실패 시 anyhow::Error
}
```

### 2. ClaudeReviewer / CodexReviewer (CLI subprocess)

```rust
pub struct ClaudeReviewer { pub model: String, pub vault_path: PathBuf }

#[async_trait]
impl WikiReviewer for ClaudeReviewer {
    async fn review(&self, page: &str, source: &str) -> Result<ReviewResult> {
        let prompt = build_user_prompt(page, source);
        let system = crate::wiki::review::load_review_system_prompt(); // task 04 가 backend suffix 추가
        let raw = run_cli(
            "claude",
            &["--model", &self.model, "--no-stream"],
            &format!("{system}\n\n{prompt}"),
            std::time::Duration::from_secs(60),
        ).await?;
        match parse_review_response(&raw) {
            Ok(r) => Ok(r),
            Err(_) => {
                // 1회 retry with strict JSON suffix
                let raw2 = run_cli(...).await?;
                parse_review_response(&raw2)
            }
        }
    }
}
```

`run_cli` helper 는 `tokio::process::Command` 사용 — `kill_on_drop(true)`, stdin pipe.

`CodexReviewer` 는 `codex` 바이너리 호출, 같은 패턴.

### 3. HaikuReviewer (Anthropic API)

`AnthropicReviewer` 와 거의 동일하지만 model_id 매칭 없음 — config 에서 받은 model 그대로 사용:

```rust
pub struct HaikuReviewer { pub api_key: String, pub model: String, pub max_tokens: u32 }

#[async_trait]
impl WikiReviewer for HaikuReviewer {
    async fn review(&self, page: &str, source: &str) -> Result<ReviewResult> {
        // POST https://api.anthropic.com/v1/messages
        // body: { model: self.model, max_tokens, system, messages: [...] }
        // parse: content[0].text → parse_review_response
    }
}
```

### 4. OllamaReviewer (HTTP `/api/chat` with `format: "json"`)

```rust
pub struct OllamaReviewer { pub api_url: String, pub model: String }

#[async_trait]
impl WikiReviewer for OllamaReviewer {
    async fn review(&self, page: &str, source: &str) -> Result<ReviewResult> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": load_review_system_prompt()},
                {"role": "user", "content": build_user_prompt(page, source)}
            ],
            "stream": false,
            "format": "json"  // ollama 의 native JSON mode
        });
        let url = format!("{}/api/chat", self.api_url.trim_end_matches('/'));
        // POST → choices[0].message.content → parse_review_response
        // 실패 시 1회 retry (동일 endpoint, prompt 에 "Output strict JSON only" suffix 추가)
    }
}
```

### 5. LmStudioReviewer (OpenAI-compat with `response_format`)

```rust
pub struct LmStudioReviewer { pub api_url: String, pub model: String }

#[async_trait]
impl WikiReviewer for LmStudioReviewer {
    async fn review(&self, page: &str, source: &str) -> Result<ReviewResult> {
        let body = json!({
            "model": self.model,
            "messages": [...],
            "response_format": { "type": "json_object" }
        });
        // POST {api_url}/v1/chat/completions
    }
}
```

### 6. retry policy

- 모든 reviewer 가 parse 실패 시 1회 retry
- retry prompt 에 `"\n\n중요: 반드시 valid JSON object 만 출력 — markdown 코드 펜스나 설명 텍스트 금지."` suffix 추가
- 2회 모두 실패 시 `anyhow::bail!("review JSON parse failed after retry")`

### 7. 회귀 테스트

`tests/wiki_reviewers.rs`:

```rust
#[tokio::test]
async fn ollama_reviewer_parses_valid_response() {
    let mut server = mockito::Server::new_async().await;
    let mock = server.mock("POST", "/api/chat")
        .with_status(200)
        .with_body(json!({
            "message": { "content": r#"{"approved":true,"issues":[]}"# }
        }).to_string())
        .create_async().await;
    let r = OllamaReviewer {
        api_url: server.url(), model: "gemma4".into()
    }.review("page", "summary").await.unwrap();
    assert!(r.approved);
    mock.assert_async().await;
}

#[tokio::test]
async fn ollama_reviewer_retries_on_parse_failure() {
    // 1차 응답: invalid (markdown wrapping)
    // 2차 응답: valid JSON
    // 결과: 성공 + retry 검증
}

#[tokio::test]
async fn lmstudio_reviewer_parses_response_format_json_object() { ... }

#[tokio::test]
async fn ollama_reviewer_fails_after_two_parse_failures() {
    // 두 응답 모두 invalid → bail
}
```

## Dependencies

- task 01 (WikiReviewer trait 정의) 필수.
- crate dep: 추가 없음. `tokio::process`, `reqwest`, `serde_json`, `async_trait`, `mockito` (dev) 모두 기존.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. 신규 회귀 테스트
cargo test -p secall-core --test wiki_reviewers

# 3. trait obj 사용 가능 — 컴파일 검증용 추가 unit test (선택)
cargo test -p secall-core --lib wiki::reviewers::

# 4. (수동) 실제 ollama
secall wiki update <session> --review --review-backend ollama
# (단, --review-backend flag 는 task 03 이후에야 동작 — 본 task 만 머지된 상태에서는 dispatcher 가 없음)
```

## Risks

- **JSON format 강제의 backend 차** — ollama 의 `format: "json"` 은 모델에 따라 무시될 수 있음 (특히 gemma 계열). retry policy 가 이를 보완하나 2회 모두 실패할 가능성 — friendly error 로 처리.
- **lmstudio 의 `response_format` 호환** — 모델이 OpenAI-compat 의 `response_format` 을 모르면 무시됨. retry 후에도 fail 하면 수동 model 변경 안내.
- **subprocess timeout 의 SIGKILL** — `tokio::process::Command::kill_on_drop(true)` 로 task 종료 시 정리. 60초는 wiki backend 의 generate 타임아웃과 동일.
- **API 변경** — Anthropic / Ollama / LmStudio 의 endpoint / payload schema 변경 시 fail. 본 task 는 현 시점 schema 기준 — 변경 시 별도 fix.
- **strict JSON 강제로 인한 quality 저하** — 모델이 json 형식 맞추느라 review 깊이 떨어질 수 있음. mitigation: system prompt 가 "issues 는 풍부하게, 단 JSON 형식 유지" 명시 (task 04).
- **`reviewers/` 모듈 분리 vs inline** — 5 backend 가 80–150줄씩 → 분리 권장. mod.rs 가 hub.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/review.rs:19-105` — task 01 의 trait + AnthropicReviewer 영역.
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — 기존 generate (wiki backend) 본체. import 만 하지 변경 X.
- `crates/secall/src/commands/wiki.rs` — task 03 영역 (호출자 dispatcher).
- `crates/secall-core/src/vault/config.rs` — task 03 / 05 영역.
- `docs/prompts/wiki-review.md` — task 04 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 06 영역.
