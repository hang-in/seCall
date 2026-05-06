# vault auto-commit 패턴 누락

- **Category**: stability (STA)
- **Severity**: minor
- **Fix Difficulty**: easy
- **Status**: resolved
- **Resolved At**: 2026-05-05
- **Resolved By**: crates/secall-core/src/vault/git.rs:146 (P39 Task 00)
- **File**: crates/secall-core/src/vault/git.rs:146

## Description

`VaultGit::auto_commit` 가 명시 패턴 (`raw/`, `wiki/`, `index.md`, `log.md`, `.gitignore`) 만 stage 하여 `SCHEMA.md`, `graph/`, `log/` 등 vault 하위 신규/변경 파일이 누락되었습니다. 매 sync 마다 일부 untracked / modified 파일이 남아 후속 `git pull --rebase` 가 "스테이징하지 않은 변경 사항이 있어 다시 적용할 수 없습니다" 로 실패하고, 사용자는 vault 디렉토리에서 수동 `git add -A && git commit` 을 반복해야 했습니다.

## Evidence

- 3회 누적 "auto: uncommitted vault changes" commit 후에도 vault 에 `M SCHEMA.md`, `?? graph/`, `?? log/` 잔존 (사용자 sync 진행 중 발견).
- 누락 패턴은 P26 ~ P37 사이 신규 산출물 (graph snapshot, log 디렉토리, SCHEMA 변경) 모두 해당.
- P39 Task 00 baseline 측정 직전 동일 증상 재현 → root cause 확정.

## Fix

`crates/secall-core/src/vault/git.rs:146` 에서 명시 패턴 stage 호출을 `git add -A` (옵션 A) 로 단일화. vault 루트 하위 모든 변경을 일괄 stage 하므로 향후 새 디렉토리 추가 시에도 패턴 갱신 불필요. 회귀 테스트는 `tests/vault_auto_commit.rs` 에 8건 추가:

- 신규 파일 stage / 수정 파일 stage / 삭제 파일 stage
- 중첩 디렉토리 / `.gitignore` 무시 동작
- 빈 변경 시 no-op
- 연속 호출 idempotency
- 기존 staged 변경과의 공존

다음 `secall sync` 부터 자동 적용. 기존 vault 에 남아 있는 unstaged 잔존은 사용자가 1회 수동 정리 (`git add -A && git commit -m "manual: post-P39 backfill" && git pull --rebase && git push`).
