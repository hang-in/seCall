---
type: plan
status: in_progress
updated_at: 2026-04-26
plan_number: P31
title: "opencode JSON 세션 ingest 지원 (#36)"
github_issue: 36
version: 1
---

# P31 — opencode JSON 세션 ingest 지원 (#36)

## Description

외부 기여자 @batmania52의 기능 요청(#36).
`opencode export`로 추출한 JSON 파일을 seCall에 ingest할 수 있도록 파서를 추가한다.

기존 `SessionParser` trait + `detect_parser()` 아키텍처를 그대로 활용하므로 사이드 이펙트가 최소화됨.
opencode는 자동 저장이 아닌 수동 export 방식이라 `--auto` 감지는 불필요하고,
기존 `path` 위치 인자(`secall ingest /path/to/file.json`)로 충분함.

## Expected Outcome

- `secall ingest /path/to/ses_abc123.json` 실행 시 opencode JSON이 파싱되어 vault에 저장됨
- `secall recall` 검색에서 opencode 세션이 조회됨
- agent 필드: `"opencode"` 고정
- `cargo test --all` + `cargo clippy --all-targets --all-features` 통과

## Subtasks

| # | Title | Files | depends_on |
|---|-------|-------|------------|
| 1 | AgentKind 확장 + opencode 파서 구현 | types.rs, opencode.rs(NEW), mod.rs | — |
| 2 | 감지 로직 통합 + 테스트 | detect.rs | Task 1 |

## Constraints

- 기존 파서(Claude, Codex, Gemini 등)에 영향 없어야 함
- CLI 인터페이스 변경 없음 — 기존 `path` 위치 인자 활용
- `parts` 배열에서 `type == "text"`만 추출 (tool-use/tool-result는 제외)

## Non-goals

- `--auto` 모드에서 opencode 세션 자동 발견 (수동 export 방식이므로 불필요)
- glob 패턴 지원 — shell expansion으로 대체 가능, 별도 이슈로 분리
- `tool-use` / `tool-result` parts 포함 — 후속 PR로 가능
