---
type: task
status: pending
plan: p31-opencode-json-ingest-36
task_number: 1
title: "AgentKind 확장 + opencode 파서 구현"
depends_on: []
parallel_group: null
---

# Task 01 — AgentKind 확장 + opencode 파서 구현

## Changed files

1. `crates/secall-core/src/ingest/types.rs:7-27` — `AgentKind` enum + `as_str()` 수정
2. `crates/secall-core/src/ingest/opencode.rs` — **NEW** opencode JSON 파서
3. `crates/secall-core/src/ingest/mod.rs:9` — 모듈 등록

## Change description

### 1. AgentKind에 OpenCode variant 추가

`crates/secall-core/src/ingest/types.rs`

```
// enum AgentKind에 추가 (line 7-14)
OpenCode,

// as_str() match arm에 추가 (line 16-27)
AgentKind::OpenCode => "opencode",
```

### 2. opencode.rs 파서 신규 작성

`crates/secall-core/src/ingest/opencode.rs` — **NEW FILE**

기존 `gemini.rs` / `codex.rs` 패턴을 따른다:

**Serde 모델** (이슈 #36의 JSON 구조 기반):

```rust
#[derive(Deserialize)]
struct OpenCodeExport {
    info: OpenCodeInfo,
    messages: Vec<OpenCodeMessage>,
}

#[derive(Deserialize)]
struct OpenCodeInfo {
    id: String,                          // session_id
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    directory: Option<String>,           // project (basename)
    #[serde(default)]
    version: Option<String>,
    time: OpenCodeTime,
}

#[derive(Deserialize)]
struct OpenCodeTime {
    created: u64,    // ms epoch → DateTime<Utc>
    #[serde(default)]
    updated: Option<u64>,
}

#[derive(Deserialize)]
struct OpenCodeMessage {
    info: OpenCodeMessageInfo,
    #[serde(default)]
    parts: Vec<OpenCodePart>,
}

#[derive(Deserialize)]
struct OpenCodeMessageInfo {
    role: String,                        // "user" | "assistant"
    #[serde(default)]
    model: Option<OpenCodeModel>,
    #[serde(default)]
    time: Option<OpenCodeMsgTime>,
}

#[derive(Deserialize)]
struct OpenCodeModel {
    #[serde(rename = "modelID", default)]
    model_id: Option<String>,
}

#[derive(Deserialize)]
struct OpenCodeMsgTime {
    #[serde(default)]
    created: Option<u64>,
}

#[derive(Deserialize)]
struct OpenCodePart {
    #[serde(rename = "type")]
    part_type: String,                   // "text", "tool-use", "tool-result", "step-start"
    #[serde(default)]
    text: Option<String>,
}
```

**파싱 로직** (`fn parse_opencode_json(path: &Path) -> Result<Session>`):

1. `std::fs::read_to_string(path)` → `serde_json::from_str::<OpenCodeExport>()`
2. `info.time.created` (ms) → `DateTime::from_timestamp_millis()` → `start_time`
3. `info.time.updated` (ms) → `end_time` (Option)
4. `info.directory` → `PathBuf` basename → `project`
5. 첫 assistant 메시지의 `model.model_id` → `session.model`
6. messages를 순회하며:
   - `parts`에서 `part_type == "text"`인 것만 필터
   - text를 join하여 Turn.content 구성
   - `info.role` → `Role::User` / `Role::Assistant` 매핑 (그 외 → `Role::System`)
   - `info.time.created` → `Turn.timestamp`
   - `Turn.index`는 0부터 순번
7. `session_type: "interactive"` (기본값)
8. `agent: AgentKind::OpenCode`

**SessionParser trait 구현**:

```rust
pub struct OpenCodeParser;

impl SessionParser for OpenCodeParser {
    fn can_parse(&self, _path: &Path) -> bool {
        false  // content-based detection만 사용, path-based 없음
    }

    fn parse(&self, path: &Path) -> crate::error::Result<Session> {
        parse_opencode_json(path).map_err(|e| crate::error::SecallError::Parse {
            path: path.to_string_lossy().into_owned(),
            source: e,
        })
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::OpenCode
    }
}
```

**1:1 파서**: opencode export는 파일 1개 = 세션 1개이므로 `parse_all()` 오버라이드 불필요.

### 3. mod.rs에 모듈 등록

`crates/secall-core/src/ingest/mod.rs` — line 9 부근에 추가:

```rust
pub mod opencode;
```

## Dependencies

- 없음 (첫 번째 태스크)

## Verification

```bash
cargo check -p secall-core
```

```bash
RUSTFLAGS="-Dwarnings" cargo clippy -p secall-core --all-targets
```

## Risks

- `DateTime::from_timestamp_millis()`는 chrono에 존재하는지 확인 필요 — chrono 0.4.26+ 지원. 현재 workspace: `chrono = "0.4"` → OK
- opencode JSON의 `info.id`가 `ses_` prefix를 포함 — 기존 DB session_id와 충돌 가능성 거의 없음 (UUID 포맷)
- `serde(rename_all)` 사용 시 opencode JSON이 camelCase인지 확인 — 이슈 #36 샘플 기준 일부 camelCase(`sessionID`, `projectID`, `modelID`), 일부 lowercase(`role`, `parts`) → 개별 `#[serde(rename)]` 사용

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/detect.rs` — Task 02에서 수정
- `crates/secall/src/commands/ingest.rs` — 변경 불필요 (1:1 파서는 기존 dispatch로 동작)
- `crates/secall/src/main.rs` — CLI 변경 없음
