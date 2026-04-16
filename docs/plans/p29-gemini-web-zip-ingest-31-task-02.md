---
type: task
plan: p29-gemini-web-zip-ingest-31
task: "02"
title: types.rs + mod.rs + detect.rs 연결
status: pending
depends_on: ["01"]
parallel_group: null
---

# Task 02 — `types.rs` + `mod.rs` + `detect.rs` 연결

## Changed files

- `crates/secall-core/src/ingest/types.rs` — `AgentKind::GeminiWeb` 배리언트 추가
- `crates/secall-core/src/ingest/mod.rs` — `pub mod gemini_web` 등록
- `crates/secall-core/src/ingest/detect.rs` — ZIP 탐지 분기에 GeminiWeb 검사 추가

## Change description

### 1. `types.rs` — `AgentKind::GeminiWeb` 추가

`crates/secall-core/src/ingest/types.rs:7` 의 `AgentKind` enum에 배리언트 추가:

```rust
pub enum AgentKind {
    ClaudeCode,
    ClaudeAi,
    ChatGpt,
    Codex,
    GeminiCli,
    GeminiWeb,   // ← 추가
}
```

`crates/secall-core/src/ingest/types.rs:17` 의 `as_str()` match에 arm 추가:

```rust
AgentKind::GeminiWeb => "gemini-web",
```

### 2. `mod.rs` — 모듈 등록

`crates/secall-core/src/ingest/mod.rs:8` (기존 `pub mod gemini;` 아래) 에 추가:

```rust
pub mod gemini_web;
```

`detect.rs`의 import에도 `GeminiWebParser` 추가 필요 (아래 Step 3에서 함께 처리).

### 3. `detect.rs` — ZIP 탐지 분기 추가

`crates/secall-core/src/ingest/detect.rs:6`의 `use super::{...}` 블록에 `gemini_web::GeminiWebParser` 추가.

`crates/secall-core/src/ingest/detect.rs:27`의 `if ext == "zip"` 블록 내부,
기존 `conversations.json` 탐지 로직(L32) **이전**에 GeminiWeb 탐지를 삽입한다.

탐지 방법: ZIP 내 첫 번째 `.json` 파일을 읽어 `projectHash` 필드 값이 `"gemini-web"`인지 확인.

```rust
// GeminiWeb: 아카이브 내 첫 .json 파일의 projectHash 확인
if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(&data)) {
    let names: Vec<String> = archive.file_names().map(|s| s.to_owned()).collect();
    if let Some(name) = names.iter().find(|n| n.ends_with(".json")) {
        if let Ok(mut f) = archive.by_name(name) {
            let mut raw = String::new();
            if std::io::Read::read_to_string(&mut f, &mut raw).is_ok() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if v["projectHash"].as_str() == Some("gemini-web") {
                        return Ok(Box::new(GeminiWebParser));
                    }
                }
            }
        }
    }
}
```

이후 기존 `conversations.json` 탐지 블록은 **변경하지 않는다**.

## Dependencies

Task 01 완료 필요 (`gemini_web.rs` 파일이 존재해야 `pub mod gemini_web` 컴파일 가능)

## Verification

```bash
cargo check -p secall-core
cargo check -p secall
```

두 명령 모두 exit 0이어야 한다.

## Risks

- `AgentKind` enum에 `GeminiWeb` 추가 시 기존 `match` 구문이 있는 파일에서 non-exhaustive 경고/에러 발생 가능
  → `cargo check`로 즉시 확인 가능. 발생 시 해당 match에 arm 추가 필요
- `detect.rs`에서 ZIP을 두 번 열게 되어 I/O 오버헤드 증가 (수백 KB 파일 기준 무시 가능)

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/gemini_web.rs` — 이미 Task 01에서 작성됨, 이 Task에서 수정 불가
- `crates/secall-core/src/ingest/gemini.rs` (기존 GeminiCli 파서)
- `crates/secall-core/src/ingest/claude_ai.rs`
- `crates/secall-core/src/ingest/chatgpt.rs`
