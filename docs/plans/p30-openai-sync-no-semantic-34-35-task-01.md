---
type: task
status: pending
updated_at: 2026-04-17
plan: p30-openai-sync-no-semantic-34-35
task_number: 1
parallel_group: A
depends_on: []
github_issue: "#35"
---

# Task 01 — OpenAI 호환 백엔드 함수 추가

## Changed files

- `crates/secall-core/src/graph/semantic.rs` — OpenAI 호환 함수 + 응답 구조체 + 디스패치 분기
- `crates/secall-core/src/vault/config.rs:156` — `semantic_backend` 주석에 `"lmstudio"` 추가
- `crates/secall/src/main.rs:322` — `--backend` 도움말에 `"lmstudio"` 추가

## Change description

### Step 1: OpenAI 응답 구조체 추가 (`semantic.rs`)

`OllamaResponse` / `OllamaMessage` (L39-44) 근처에 OpenAI ChatCompletion 응답 구조체를 추가한다:

```
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    content: String,
}
```

### Step 2: `extract_with_openai_compat` 함수 추가 (`semantic.rs`)

`extract_with_ollama` (L166-204) 직후에 새 함수를 추가한다:

```
async fn extract_with_openai_compat(
    fm: &SessionFrontmatter,
    body: &str,
    base_url: &str,
    model: &str,
) -> Result<Vec<GraphEdge>>
```

핵심 차이점 (vs `extract_with_ollama`):
- **URL**: `{base_url}/v1/chat/completions` (Ollama는 `/api/chat`)
- **요청 포맷**: OpenAI ChatCompletion (`model`, `temperature`, `messages` — `stream`과 `options` 대신 top-level `temperature`)
- **응답 파싱**: `OpenAIResponse` → `choices[0].message.content` (Ollama는 `message.content` 직접)
- 나머지(프롬프트, 파싱)는 동일하게 `build_user_content` + `parse_llm_edges` 사용

요청 바디 형태:
```json
{
  "model": "<model>",
  "temperature": 0.1,
  "messages": [
    {"role": "system", "content": SYSTEM_PROMPT},
    {"role": "user", "content": user_content}
  ]
}
```

에러 처리는 `extract_with_ollama`와 동일 패턴:
- `!resp.status().is_success()` → `anyhow::bail!("OpenAI-compat API error {}: {}", status, text)`
- `choices`가 비어있으면 `anyhow::bail!("OpenAI-compat API returned empty choices")`

### Step 3: `extract_with_llm` 디스패치에 `"lmstudio"` 분기 추가 (`semantic.rs:352`)

현재 match 분기:
```
"ollama" => ...
"anthropic" => ...
"gemini" => ...
_ => anyhow::bail!(...)
```

`"gemini"` 뒤, `_` 앞에 추가:
```
"lmstudio" => {
    let base_url = config.ollama_url.as_deref().unwrap_or("http://localhost:1234");
    let model = config.ollama_model.as_deref().unwrap_or("gemma-4-e4b-it");
    extract_with_openai_compat(fm, body, base_url, model).await
}
```

> 기본 URL이 Ollama(`11434`)가 아닌 LM Studio(`1234`)임에 주의.

### Step 4: 주석/도움말 업데이트

- `config.rs:156` — `semantic_backend` 주석: `"ollama" | "anthropic" | "gemini" | "lmstudio" | "disabled"`
- `main.rs:322` — `--backend` arg 도움말: `"ollama" | "gemini" | "anthropic" | "lmstudio" | "disabled"`

## Dependencies

없음 (독립 태스크)

## Verification

```bash
# 1. 타입 체크
cargo check -p secall-core -p secall

# 2. 기존 테스트 통과 확인
cargo test -p secall-core

# 3. CLI 도움말에 lmstudio 표시 확인
cargo run -- graph semantic --help 2>&1 | grep -i lmstudio
```

## Risks

- **낮음**: 새 함수 + 새 match 분기 추가만이므로 기존 ollama/anthropic/gemini 코드에 영향 없음
- `OpenAIResponse` 구조체가 LM Studio 실제 응답과 불일치할 가능성 — LM Studio는 OpenAI 호환이므로 `choices[0].message.content` 경로는 안전. 단, `usage` 등 추가 필드는 `#[derive(Deserialize)]`에서 자동 무시됨 (serde 기본 동작)

## Scope boundary (수정 금지)

- `commands/sync.rs` — Task 02 영역
- `commands/ingest.rs` — 기존 코드
- `extract_with_ollama`, `extract_with_gemini`, `extract_with_anthropic` 함수 본문 — 기존 백엔드 건드리지 않음
