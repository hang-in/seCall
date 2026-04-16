---
type: task
plan: p29-gemini-web-zip-ingest-31
task: "03"
title: 단위 테스트
status: pending
depends_on: ["01", "02"]
parallel_group: null
---

# Task 03 — 단위 테스트

## Changed files

- `crates/secall-core/src/ingest/gemini_web.rs` — `#[cfg(test)] mod tests` 섹션 추가

## Change description

`gemini_web.rs` 파일 하단에 인라인 테스트를 추가한다.
테스트 fixture는 이슈 #31의 JSON 예시를 기반으로 하며, 코드 내 문자열 리터럴로 삽입한다.

### 테스트 1 — `test_json_to_session_basic`

단일 JSON 문자열(`SAMPLE_JSON`)을 `serde_json::from_str`로 파싱한 뒤
`json_to_session()`을 호출하여 다음을 검증한다:

- `session.id == "22836c74f2ebe4cc"`
- `session.agent == AgentKind::GeminiWeb`
- `session.turns.len() == 2`
- `session.turns[0].role == Role::User`
- `session.turns[0].content == "내일 날씨 알려줘"`
- `session.turns[1].role == Role::Assistant`
- `session.model == Some("2.5 Flash".to_string())`

Fixture (`SAMPLE_JSON`):

```json
{
  "sessionId": "22836c74f2ebe4cc",
  "title": "부천시 내일 날씨 예보",
  "startTime": "2025-07-24T08:19:17.000Z",
  "lastUpdated": "2025-07-24T08:19:33.000Z",
  "kind": "main",
  "projectHash": "gemini-web",
  "messages": [
    {
      "id": "msg-0",
      "timestamp": "2025-07-24T08:19:17.000Z",
      "type": "user",
      "content": [{ "text": "내일 날씨 알려줘" }]
    },
    {
      "id": "msg-1",
      "timestamp": "2025-07-24T08:19:17.000Z",
      "type": "gemini",
      "content": "내일 부천시는 맑은 하늘이 예상되며, 최고 기온은 34도입니다.",
      "model": "2.5 Flash"
    }
  ]
}
```

### 테스트 2 — `test_parse_all_from_zip`

두 개의 JSON 파일을 포함하는 인메모리 ZIP을 `zip::ZipWriter`로 생성한 뒤
임시 파일로 저장하고 `GeminiWebParser.parse_all()`을 호출하여:

- 반환된 `Vec<Session>.len() == 2` 검증

ZIP 생성 패턴:

```rust
use std::io::Write;
let mut buf = Vec::new();
let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
let opts = zip::write::FileOptions::default();
zip.start_file("session1.json", opts).unwrap();
zip.write_all(SAMPLE_JSON.as_bytes()).unwrap();
zip.start_file("session2.json", opts).unwrap();
zip.write_all(SAMPLE_JSON2.as_bytes()).unwrap(); // sessionId만 다른 fixture
zip.finish().unwrap();
// buf를 tempfile에 쓰거나 직접 ZipArchive::new(Cursor::new(&buf))로 테스트
```

`GeminiWebParser.parse_all()`이 `&Path`를 받으므로,
`tempfile::NamedTempFile`을 사용하거나,
`zip::ZipArchive`를 직접 사용하는 내부 헬퍼 함수를 `parse_all()`에서 분리하여
`parse_archive(archive: ZipArchive<R>) -> Result<Vec<Session>>` 형태로 테스트한다.

> 참고: `zip` crate는 이미 `Cargo.toml`에 의존성이 있으므로 추가 불필요.
> `tempfile` crate가 없으면 `zip::ZipArchive::new(Cursor::new(buf))`로 직접 처리하거나
> 내부 `parse_archive` 헬퍼를 분리하여 테스트하는 방식을 사용한다.

## Dependencies

Task 01, Task 02 완료 필요

## Verification

```bash
cargo test -p secall-core ingest::gemini_web
```

`test_json_to_session_basic`과 `test_parse_all_from_zip` 두 테스트 모두 `ok` 출력 확인.

## Risks

- `tempfile` crate가 dev-dependency에 없을 경우 인메모리 ZIP + `Cursor` 기반 내부 헬퍼 테스트로 대체
- `zip::ZipWriter::FileOptions` API는 zip crate 버전에 따라 다를 수 있음
  → `Cargo.toml`의 현재 zip 버전 확인 후 API 맞춤 필요

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/types.rs`
- `crates/secall-core/src/ingest/mod.rs`
- `crates/secall-core/src/ingest/detect.rs`
- 기타 기존 파서 파일 전체
