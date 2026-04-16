---
type: task
plan: p29-gemini-web-zip-ingest-31
task: "01"
title: gemini_web.rs 파서 구현
status: pending
depends_on: []
parallel_group: null
---

# Task 01 — `gemini_web.rs` 파서 구현

## Changed files

- **신규**: `crates/secall-core/src/ingest/gemini_web.rs`

## Change description

Gemini Web ZIP 포맷을 `Session`으로 변환하는 파서를 신규 파일로 구현한다.

### 1. JSON 구조체 정의

이슈 #31의 포맷 명세를 기준으로 serde 구조체를 정의한다.

```
GeminiWebExport {
    session_id: String,        // "sessionId"
    title: Option<String>,
    start_time: String,        // ISO 8601
    last_updated: String,      // ISO 8601
    messages: Vec<GeminiWebMessage>,
}

GeminiWebMessage {
    id: String,
    timestamp: String,         // ISO 8601
    msg_type: String,          // "user" | "gemini"  (필드명: "type")
    content: GeminiWebContent, // serde untagged enum
    model: Option<String>,
}

GeminiWebContent = Text(String) | Parts(Vec<GeminiWebPart>)
GeminiWebPart { text: String }
```

`content` 필드는 user 메시지(배열)와 gemini 응답(문자열)이 다르므로 `#[serde(untagged)]` enum으로 처리한다.

### 2. `json_to_session()` 함수

`GeminiWebExport` → `Session` 변환:

- `session.id` = `export.session_id`
- `session.agent` = `AgentKind::GeminiWeb` (Task 02에서 추가됨)
- `session.model` = 마지막 gemini 메시지의 `model` 필드
- `session.project` = `None` (Gemini Web은 project 정보 없음)
- `session.start_time` = `start_time` 파싱 (파싱 실패 시 `Utc::now()`)
- `session.end_time` = `last_updated` 파싱
- `session.session_type` = `"interactive"`
- turns: messages 배열을 순회하며 `Turn` 생성
  - `role`: `"user"` → `Role::User`, `"gemini"` → `Role::Assistant`, 그 외 → `Role::System`
  - `content`: `Text(s)` → `s`, `Parts(v)` → `v`의 text 필드들을 `\n`으로 join
  - `actions`: `Vec::new()`
  - `is_sidechain`: `false`

### 3. `GeminiWebParser` 구조체 + `SessionParser` 구현

```
pub struct GeminiWebParser;

impl SessionParser for GeminiWebParser {
    fn can_parse(&self, path: &Path) -> bool
        // ext == "zip"만 true
    fn parse(&self, path: &Path) -> Result<Session>
        // parse_all()의 첫 번째 항목 반환
    fn parse_all(&self, path: &Path) -> Result<Vec<Session>>
        // ZIP 열기 → 파일명 목록 수집 → .json 파일만 순회 → json_to_session() 호출
        // 개별 파일 파싱 실패 시 tracing::warn 후 skip (전체 실패 아님)
    fn agent_kind(&self) -> AgentKind
        // AgentKind::GeminiWeb
}
```

`parse_all()` ZIP 처리는 `claude_ai.rs`의 `extract_conversations_from_zip()` 패턴을 참고하되,
단일 파일이 아닌 아카이브 내 전체 `.json` 파일을 순회하도록 구현한다.

```rust
// ZIP 파일명 목록 수집 후 순회 패턴 (file_names()은 &str 반환)
let names: Vec<String> = archive.file_names().map(|s| s.to_owned()).collect();
for name in &names {
    if !name.ends_with(".json") { continue; }
    let mut file = archive.by_name(name)?;
    // ...
}
```

`zip::ZipArchive`는 `by_name()` 호출 중 borrow 충돌이 있으므로
파일명을 먼저 `Vec<String>`으로 수집한 뒤 순회하는 패턴을 반드시 사용한다.

## Dependencies

없음 (신규 파일, 독립 구현 가능)
단, `AgentKind::GeminiWeb`은 Task 02에서 추가되므로 컴파일은 Task 02 완료 후 통과한다.

## Verification

```bash
# Task 02 완료 후 실행 가능
cargo check -p secall-core
```

## Risks

- `content` 필드의 `untagged` enum 파싱 실패 시 전체 세션 skip → warn 로그로 추적 가능
- ZIP 내 `.json`이 아닌 파일(예: `__MACOSX/`) 포함 가능 → `.json` 확장자 필터로 방지
- `zip::ZipArchive::by_name()` borrow 충돌 → 파일명 사전 수집으로 방지

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/gemini.rs` (기존 GeminiCli 파서)
- `crates/secall-core/src/ingest/detect.rs` (Task 02 담당)
- `crates/secall-core/src/ingest/mod.rs` (Task 02 담당)
- `crates/secall-core/src/ingest/types.rs` (Task 02 담당)
