# seCall — Agent Instructions

> This file defines project-level rules for all agents in tunaFlow.
> All agents (Claude, Gemini, Codex, OpenCode) must follow these rules.

---

## 1. Project Overview

- Name: seCall
- Status: active
- Language: Rust
- Test: `cargo test`
- Type check: `cargo check`
- Stack: Rust

> Auto-detected by tunaFlow. Verify and adjust if needed.

---

## 2. File Storage Rules

**All documents and artifacts must be created within this project directory.**

- Do NOT create files in `~/.claude/`, `~/.gemini/`, or any external path
- Plans: `docs/plans/`
- Reference docs: `docs/reference/`
- Prompts: `docs/prompts/`
- Code: follow project structure

---

## 3. Documentation Rules

### File Naming
- Short, 2-4 core tokens (camelCase)
- Reference: stable names without dates (e.g., `implementationStatus.md`)
- Plan: `featureNamePlan.md` or `featureNamePlan_YYYY-MM-DD.md`
- Prompt: `docs/prompts/YYYY-MM-DD/short_name.md`

### Document Metadata
- Top of every document: `type`, `status`, `updated_at`
- Status values: `draft` → `in_progress` → `done` → `archived`
- Reference docs: update same file (no date-based duplication)
- Plans/prompts: new documents per task allowed (must update index.md)

### Versioning
- Use `status: archived` + `superseded_by` instead of deletion
- Brainstorm/comparison docs: mark `canonical: false`

---

## 4. Coding Rules

### Language
- Respond in the language the user uses (match user's message language)
- Code, paths, identifiers: keep in original language

### Code Quality
- Only modify what was requested. Do not clean up surrounding code
- Error handling: minimize silent fallbacks during development
- No speculative abstractions or future-proofing
- Modify one path at a time → verify → proceed to next

### Testing
- Verify existing tests pass after changes
- Consider unit tests for new logic

---

## 5. Work Safety Rules

- **Verify replacement works** before removing existing functionality
- **Confirm before destructive operations** (file deletion, schema changes)
- **Single-path modification** — never change multiple execution paths simultaneously
- Check all consumers before modifying shared state

---

## 6. Agent Behavior Rules

- **Plan before implementing** — present your plan and wait for user approval before writing code
- Introduce yourself by profile name first, then engine. No mixed expressions
- Do not claim ownership of other agents' messages
- Respond in the user's language
- Lead with conclusions, then reasoning

---

## 7. Current Status

### Completed
- P18 Rev.2 — 세션 분류 regex 사전 컴파일 및 에러 전파
- P22 Rev.2 — Wiki 파이프라인 (Haiku 생성, lint, review + auto-retry)
- Semantic graph extraction — 694세션 완료 (348 skipped, 0 failed)
- P23 — Store/Search 모듈 경계 리팩토링 (search 모듈에서 SQL 분리)
- P24 — GitHub 이슈 일괄 수정 (#19 timezone, #21 local-only, #22 compact turn, #23 FTS5 중복)
- PR #20 — OpenVINO embedding backend (외부 기여, CoLuthien)
- P25 Phase 0-1 — REST API 서버 + Obsidian 플러그인 MVP (PR #24)
- P25 Phase 2 — 데일리 노트 자동 생성 + Graph 탐색 뷰 (PR #27)
- P27 — BM25-only 선택 시 graph semantic 자동 비활성화 (#25 fix, PR #27)
- wiki 기본 생성 백엔드 claude → codex 강등 (claude -p 막힘 + 빌링 fragile, claude 코드는 유지, PR #111)
- v0.6.4 릴리스 — sync 시맨틱 추출 진행 CLI 출력(#112) + codex 기본값(#111) 포함 (#113, tag v0.6.4)
- PR #108 — web UI 세션 삭제 (외부 기여 kainy21, Windows 에서 머지)
- PR #115 — zero-turn 세션 비파괴 healing (`reindex --repair-missing-turns` + embed vault 폴백 + 경로 canonical, Gemini 리뷰 3건 반영)
- PR #116 — README 슬림화(798→729줄) + API 문서 분리(`docs/reference/api.md`) + 버전이력 CHANGELOG 통합
- CodeRabbit 자동 PR 리뷰 도입 — Gemini Code Assist 2026-07-17 종료 대체 (공개 레포 무료). PR #117 로 동작 검증 (CodeRabbit + Gemini 둘 다 동일 SSE 오류 잡음). **P3 해결.**

### In Progress
- (없음) — Open PR 없음. 다음 우선순위는 §8 참고.
  (P26 Gemini API 백엔드는 폐기 — PR #110 에서 dead doc 잔재까지 제거됨)

### Known Issues
- 기존 DB에 FTS 중복 잔존 (--force reingest로 세션별 정리 가능)
- Issue #26 — Codex wiki 백엔드 추가 (외부 기여 PR 요청 중)

---

## 8. Next Priorities

1. (P1) 배포방법 고도화 — Linux 바이너리 / cargo-binstall / 체크섬 검증 등 (상세: `docs/reference/handoff_2026-06-26.md` §3.1, 최신 맥락: `handoff_2026-07-02.md`)
2. (P2) 테스트 갭 대응 — REST API DTO/라우터 등 미테스트 46건

> (해결) ~~P3 Gemini Code Assist 7/17 종료 대비~~ → **CodeRabbit 도입으로 대체** (공개 레포 무료, PR #117 검증). Gemini 는 7/17 자연 종료 예정 — 별도 제거 불필요.

---

## 9. Git Workflow

- 새 기능/수정은 **feature branch**에서 작업 → PR → merge 패턴을 따름
- 브랜치명: `feat/p25-wiki-dryrun`, `fix/issue-name` 등
- PR에서 관련 이슈 자동 close: `Fixes #19, #21` 등 사용
- 외부 기여 PR은 리뷰 후 필요 시 직접 수정 커밋 추가 → merge → 코멘트
