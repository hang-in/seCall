---
type: plan
status: in_progress
updated_at: 2026-04-16
issue: "hang-in/seCall#31"
---

# P29 — Gemini Web 대화 ZIP ingest 지원 (#31)

## Description

batmania52의 gemini-exporter Chrome 확장이 생성하는 ZIP 파일을
`secall ingest path/to/gemini-export.zip` 명령으로 처리할 수 있도록 지원한다.

ZIP 내에는 대화 1건당 JSON 파일 1개가 들어 있으며, 각 파일은 `projectHash: "gemini-web"` 필드를 포함한다.
이 필드로 기존 claude.ai / ChatGPT ZIP과 구분한다.

## Expected Outcome

- `secall ingest gemini-export.zip` 실행 시 ZIP 내 모든 JSON 파일이 `Session`으로 파싱되어 vault에 저장됨
- `AgentKind::GeminiWeb` 태깅으로 기존 `GeminiCli` 세션과 구분됨
- 기존 claude.ai / ChatGPT ZIP 탐지 로직 미영향

## Subtasks

| # | 파일 | 내용 |
|---|------|------|
| 01 | `ingest/gemini_web.rs` (신규) | JSON 구조체 + `parse_all()` 구현 |
| 02 | `types.rs`, `mod.rs`, `detect.rs` | `GeminiWeb` 배리언트 등록 + ZIP 탐지 분기 추가 |
| 03 | `ingest/gemini_web.rs` (테스트 섹션) | 인라인 단위 테스트 |

## Constraints

- 기존 `GeminiParser`(CLI JSON) 수정 금지
- `detect.rs`의 claude.ai / ChatGPT 탐지 분기 로직 변경 금지

## Non-goals

- gemini-exporter Chrome 확장 자체 개발
- Gemini Web 이미지/첨부파일 렌더링
- 기존 `~/.gemini/` 디렉토리 자동 스캔 경로 변경
