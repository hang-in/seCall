---
type: task
status: pending
plan: p31-opencode-json-ingest-36
task_number: 2
title: "감지 로직 통합 + 테스트"
depends_on: [1]
parallel_group: null
---

# Task 02 — 감지 로직 통합 + 테스트

## Changed files

1. `crates/secall-core/src/ingest/detect.rs:6-9` — import에 `OpenCodeParser` 추가
2. `crates/secall-core/src/ingest/detect.rs:92-105` — content sniffing에 opencode 감지 로직 추가

## Change description

### 1. import 추가

`crates/secall-core/src/ingest/detect.rs` line 6-9:

기존 import 블록에 `opencode::OpenCodeParser` 추가:

```rust
use super::{
    chatgpt::ChatGptParser, claude::ClaudeCodeParser, claude_ai::ClaudeAiParser,
    codex::CodexParser, gemini::GeminiParser, gemini_web::GeminiWebParser,
    opencode::OpenCodeParser, SessionParser,
};
```

### 2. content sniffing에 opencode 감지 추가

`detect.rs`의 content sniffing 섹션 (line 92 부근, `first_line.trim_start().starts_with('{')` 블록 내부).

**감지 로직**: Gemini 감지(line 97-99)보다 **앞에** opencode 감지를 삽입한다.
이유: Gemini도 `messages` 배열을 가지지만, opencode는 `info.id` + `messages[].parts` 구조가 고유하다.

```rust
// 기존 코드: if first_line.trim_start().starts_with('{') { ... }
// 그 블록 안에서 full parse 후:

// opencode: "info" object with "id" + "messages" array
if v["info"]["id"].is_string() && v["messages"].is_array() {
    return Ok(Box::new(OpenCodeParser));
}

// 기존 Gemini 감지 (이 뒤에 위치)
if v["messages"].is_array() && v["messages"][0]["parts"].is_array() {
    return Ok(Box::new(GeminiParser));
}
```

**감지 우선순위 근거**:
- opencode JSON은 최상위에 `info` 객체(id, title, directory 등)가 존재 — Gemini에는 없음
- `v["info"]["id"].is_string()` 체크가 opencode에만 고유하므로 false positive 없음
- Gemini는 `messages[0].parts`로 구분되므로 opencode와 충돌하지 않음

### 3. 테스트 추가

`detect.rs`의 `#[cfg(test)] mod tests` 블록 (line 247~)에 테스트 추가:

```rust
#[test]
fn test_detect_opencode_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ses_abc123.json");
    std::fs::write(
        &path,
        r#"{
          "info": {
            "id": "ses_abc123",
            "slug": "test-session",
            "projectID": "proj1",
            "directory": "/Users/user/projects/myapp",
            "title": "Test session",
            "version": "1.14.24",
            "time": { "created": 1777090810040, "updated": 1777091142209 }
          },
          "messages": [
            {
              "info": {
                "role": "user",
                "id": "msg_001",
                "sessionID": "ses_abc123",
                "time": { "created": 1777090810253 }
              },
              "parts": [
                { "type": "text", "text": "Hello", "id": "prt_001", "sessionID": "ses_abc123", "messageID": "msg_001" }
              ]
            },
            {
              "info": {
                "role": "assistant",
                "id": "msg_002",
                "sessionID": "ses_abc123",
                "model": { "providerID": "llama", "modelID": "Qwen3.6-35B" },
                "time": { "created": 1777090820000 }
              },
              "parts": [
                { "type": "step-start", "snapshot": {} },
                { "type": "text", "text": "Hi there!", "id": "prt_002", "sessionID": "ses_abc123", "messageID": "msg_002" }
              ]
            }
          ]
        }"#,
    )
    .unwrap();

    let parser = detect_parser(&path).unwrap();
    assert_eq!(parser.agent_kind(), super::super::types::AgentKind::OpenCode);
}
```

추가로, `opencode.rs` 내부에 파서 단위 테스트도 작성:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_opencode_session() {
        // 위와 동일한 JSON → parse → Session 필드 검증
        // - session.id == "ses_abc123"
        // - session.agent == AgentKind::OpenCode
        // - session.project == Some("myapp")
        // - session.turns.len() == 2
        // - turns[0].role == Role::User, turns[0].content == "Hello"
        // - turns[1].role == Role::Assistant, turns[1].content == "Hi there!"
        // - turns[1].content에 "step-start" 텍스트 미포함 (type=text만 추출)
    }
}
```

## Dependencies

- Task 01 (AgentKind::OpenCode + opencode.rs 파서 존재해야 함)

## Verification

```bash
cargo test -p secall-core -- detect::tests::test_detect_opencode_json --exact
```

```bash
cargo test -p secall-core -- opencode::tests --exact
```

```bash
cargo test --all
```

```bash
RUSTFLAGS="-Dwarnings" cargo clippy --all-targets --all-features
```

## Risks

- **감지 순서**: opencode 감지가 Gemini보다 앞에 와야 함. Gemini의 `messages[0].parts` 체크는 opencode도 통과할 수 있으므로, `info.id` 체크를 선행해야 오탐 방지
- **기존 테스트 영향**: opencode 감지는 기존 JSON 구조(`sessionId`, `conversation_id`, `uuid` 등)와 겹치지 않으므로 기존 테스트에 영향 없음

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/types.rs` — Task 01에서 완료
- `crates/secall-core/src/ingest/opencode.rs` — Task 01에서 생성 (단, 이 Task에서 `#[cfg(test)]` 블록 추가는 허용)
- `crates/secall-core/src/ingest/mod.rs` — Task 01에서 완료
- `crates/secall/src/commands/ingest.rs` — 변경 불필요
- `crates/secall/src/main.rs` — CLI 변경 없음
