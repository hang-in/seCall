---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 04
title: review prompt JSON 강제 + backend suffix
parallel_group: B
depends_on: [01]
status: pending
updated_at: 2026-05-09
---

# Task 04 — review prompt JSON 강제 + backend suffix

## Changed files

수정:
- `docs/prompts/wiki-review.md` — 본문 끝에 "출력 형식" 섹션 추가 (현재 본문 유지 + suffix 만 보강). 명시적으로 valid JSON object 출력만 허용 + markdown fence 금지 + 예시 1개.
- `crates/secall-core/src/wiki/review.rs:108-119` 의 `load_review_system_prompt()` 시그니처를 `load_review_system_prompt(backend: ReviewerKind) -> String` 으로 변경. backend 종류에 따라 strict-JSON suffix 강도 조정.

신규:
- `crates/secall-core/src/wiki/review.rs` 안에 `pub enum ReviewerKind { Anthropic, Claude, Codex, Haiku, Ollama, LmStudio }` 추가. backend 별 prompt suffix 분기에 사용.
- `docs/prompts/wiki-review-strict-json.md` (신규) — local backend (ollama / lmstudio) 용 추가 suffix. 본문은 짧음 (10–20줄): JSON 형식 강제 + 예시.

회귀 테스트:
- `crates/secall-core/src/wiki/review.rs` 의 `#[cfg(test)] mod tests` 에 4 test fn 추가:
  1. `prompt_loads_for_anthropic_kind` — non-empty + 핵심 토큰 ("위키 페이지 검수" 같은 식별자) 포함.
  2. `prompt_for_local_backends_includes_strict_json_suffix` — `Ollama` / `LmStudio` 의 suffix 가 합쳐진 prompt 길이 > anthropic 길이.
  3. `prompt_loads_external_file_when_present` — 임시 파일 + env 통한 path override (`SECALL_WIKI_REVIEW_PROMPT`) 사용 시 그 내용 반환.
  4. `prompt_falls_back_to_embedded_when_external_missing` — 외부 path 없는 환경에서 `include_str!` fallback 동작.

## Change description

### 1. ReviewerKind enum

```rust
// review.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerKind {
    Anthropic,
    Claude,
    Codex,
    Haiku,
    Ollama,
    LmStudio,
}
```

각 reviewer impl (task 02) 에서 `load_review_system_prompt(ReviewerKind::Ollama)` 등으로 호출.
`AnthropicReviewer` (task 01 의 본체) 도 `ReviewerKind::Anthropic` 로 호출하도록 수정.

### 2. load_review_system_prompt 시그니처

```rust
pub fn load_review_system_prompt(kind: ReviewerKind) -> String {
    let base = load_base_prompt(); // 기존 외부 파일 / embedded fallback 로직
    match kind {
        ReviewerKind::Ollama | ReviewerKind::LmStudio => {
            // local backend: strict JSON suffix 추가
            format!("{base}\n\n{}", load_strict_json_suffix())
        }
        _ => base,
    }
}

fn load_base_prompt() -> String {
    // 기존 review.rs:108-119 의 본체 — 외부 path → embedded fallback
}

fn load_strict_json_suffix() -> String {
    // SECALL_VAULT_PATH/wiki-review-strict-json.md 우선,
    // 없으면 include_str!("../../../../docs/prompts/wiki-review-strict-json.md")
}
```

### 3. wiki-review.md 본문 보강

기존 본문 끝에 추가:

```markdown

## 출력 형식

응답은 **valid JSON object 단일** 만 포함해야 합니다.
markdown 코드 펜스, 설명 텍스트, 주석 모두 금지.

JSON schema:

\`\`\`
{
  "approved": boolean,
  "issues": [
    { "severity": "critical|major|minor", "description": string, "suggestion": string|null }
  ]
}
\`\`\`

승인된 경우: `{"approved": true, "issues": []}` 만 반환.
```

(코드 펜스 자체는 docs 안에서 escape 또는 ASCII art 로 표현.)

### 4. wiki-review-strict-json.md 신규

```markdown
중요: 응답은 valid JSON object 형식만 허용합니다.

- markdown 코드 펜스 (\`\`\`json ... \`\`\`) 금지
- "다음은 결과입니다:" 같은 설명 텍스트 금지
- 주석 (// 또는 /* */) 금지
- approved 가 true 인 경우에도 issues 배열은 항상 포함 (빈 배열 가능)

좋은 예: {"approved":true,"issues":[]}
나쁜 예: ```json
{"approved":true,"issues":[]}
```
```

### 5. 외부 path override

기존 `load_base_prompt` 가 vault path 기준으로 외부 파일 찾음. 본 task 는 해당 동작 유지 + 새 env override 추가:

```rust
fn load_base_prompt() -> String {
    if let Ok(path) = std::env::var("SECALL_WIKI_REVIEW_PROMPT") {
        if let Ok(s) = std::fs::read_to_string(&path) {
            return s;
        }
    }
    // 기존 vault-relative 경로 시도
    // ...
    // include_str! fallback
    include_str!("../../../../docs/prompts/wiki-review.md").to_string()
}
```

테스트에서 임시 파일 생성 + env var 설정 → fallback chain 검증에 사용.

### 6. 호출자 갱신

task 02 의 5 reviewer 가 `load_review_system_prompt(ReviewerKind::Xxx)` 사용 (task 02 와 동기 머지).
task 01 의 `AnthropicReviewer` 도 `ReviewerKind::Anthropic` 로 호출하도록 수정.

호출자 변경은 본 task 가 함수 시그니처 변경 → task 02 / task 01 의 후속 적용.

## Dependencies

- task 01 (WikiReviewer trait + AnthropicReviewer 분리) 필수 — `ReviewerKind` 의 위치가 review.rs.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. prompt 로딩 회귀
cargo test -p secall-core --lib wiki::review::tests

# 3. external override 동작 (수동)
SECALL_WIKI_REVIEW_PROMPT=/tmp/custom.md cargo test -p secall-core --lib wiki::review::tests::prompt_loads_external_file_when_present

# 4. embedded fallback (수동, env 미설정)
unset SECALL_WIKI_REVIEW_PROMPT
cargo test -p secall-core --lib wiki::review::tests::prompt_falls_back_to_embedded_when_external_missing

# 5. 본문 검증
grep -c "valid JSON object" docs/prompts/wiki-review.md   # 1 이상
grep -c "markdown 코드 펜스" docs/prompts/wiki-review-strict-json.md   # 1 이상
```

## Risks

- **prompt 길이 증가의 토큰 비용** — local backend 의 경우 suffix 추가로 input 토큰 +200. wiki review 자체가 batch 호출이라 비용 영향 미미.
- **모델별 JSON 신뢰성 차** — gemma 계열 ollama 모델은 suffix 가 있어도 markdown 으로 wrapping 자주. task 02 의 retry policy 가 이를 보완.
- **외부 path override 의 보안** — `SECALL_WIKI_REVIEW_PROMPT` 가 임의 경로 읽음. 멀티유저 환경에서는 path traversal 위험 — 본 plan 은 단일 사용자 가정. 향후 path canonicalize + allowlist 별도 plan.
- **prompt 본문 변경의 review 품질 영향** — 본 task 는 suffix 만 추가. 본문 자체는 변경 X.
- **`include_str!` 경로의 build-time 의존** — `docs/prompts/wiki-review.md` / `wiki-review-strict-json.md` 둘 다 build 시 존재해야 함. 본 task 가 둘 다 만듦.
- **task 02 와의 머지 race** — 5 reviewer impl (task 02) 가 `ReviewerKind` enum 사용. 본 task 가 enum 정의 → task 02 가 사용. parallel_group B 동시 진행 가능 — task 01 머지 후 두 task 분담.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/reviewers/*.rs` — task 02 영역 (호출만, body 변경 X).
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — wiki backend (review 와 별도).
- `crates/secall/src/commands/wiki.rs` — task 03 영역.
- `crates/secall-core/src/vault/config.rs` — task 03 / 05 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 06 영역.
