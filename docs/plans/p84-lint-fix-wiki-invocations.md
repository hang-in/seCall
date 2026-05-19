---
type: plan
status: in_progress
updated_at: 2026-05-19
canonical: true
---

# P84 — `secall lint --fix-wiki-invocations` (P83 fast-follow)

## 배경

P83 (#85) 머지로 신규 codex/claude wiki invocation 세션은 `is_noise_session()` 의 marker 검사로 차단되지만, **머지 전에 이미 ingest 된** 세션에는 marker 가 없어 자동 정리가 안 된다. dicebattle (issue #82) 등 사용자가 수동으로 `secall archive` 또는 `~/.codex/sessions/` 파일 삭제로 정리해야 하는 부담.

P83 PR 의 "Out of scope" 에서 fast-follow PR 로 분리 명시.

## 목표

- 사용자가 `secall lint --fix-wiki-invocations` 한 번 실행해 legacy wiki invocation 세션을 일괄 archive.
- archive 는 reversible (`secall unarchive`) 이라 false positive 시에도 복구 가능.

## 비목표

- 새 ingest 동작 변경 없음 (P83 marker 검사가 이미 처리).
- 자동 sync --force 등 사용자 모르게 동작 안 함 — 명시적 명령으로만.

## 검출 룰 (L011 신규)

```
SELECT id, cwd, agent FROM sessions
WHERE is_archived = 0
  AND cwd IS NOT NULL
  AND agent IN ('codex', 'claude-code')
```
→ `cwd == config.vault.path` 인 경우 L011 finding (Severity: Info).

근거: `wiki/codex.rs` + `wiki/claude.rs` 가 subprocess 를 `cwd = vault_path` 으로 spawn. 따라서 wiki 호출이 만든 세션의 cwd 는 정확히 vault path. 사용자가 vault 디렉토리 자체에서 일반 작업할 확률은 매우 낮음 (vault 는 wiki 저장소).

## 사용 흐름

```bash
# 1. 검사 (자동 fix 없이)
secall lint
# → L011 [INFO] codex session at vault path (likely wiki self-invocation): cwd=/Users/me/Documents/Obsidian Vault/seCall
# → L011 [INFO] claude-code session at vault path ...

# 2. 자동 archive
secall lint --fix-wiki-invocations
# → [fix-wiki-invocations] Archiving 12 wiki invocation session(s)...
# →   archived a1b2c3d4
# →   ...
# → [fix-wiki-invocations] Done. 12 archived, 0 failed.

# 3. (의도와 다르면 개별 복원)
secall unarchive <session-id>
```

## 변경 파일

| 파일 | 변경 |
|---|---|
| `crates/secall-core/src/ingest/lint.rs` | `check_wiki_invocations()` 함수 추가, `run_lint` 에 호출, 신규 unit test 5건 |
| `crates/secall/src/commands/lint.rs` | `run_fix_wiki_invocations()` 함수 추가, `run()` 시그니처에 `fix_wiki_invocations: bool` |
| `crates/secall/src/main.rs` | clap arg `--fix-wiki-invocations` + 호출 사이트 갱신 |
| `docs/plans/index.md` | P84 등록 |

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p secall-core --lib ingest::lint::tests
cargo test --workspace --no-fail-fast
```

## 리스크

- **False positive**: 사용자가 vault 디렉토리에서 codex/claude 일반 작업한 경우 archive 됨. 대응: `secall unarchive` 로 복구. 게다가 vault 는 일반적으로 wiki 저장소라 직접 작업 빈도 낮음.
- **agent 이름 정확성**: SQL 의 `IN ('codex', 'claude-code')` 가 `AgentKind::as_str()` 값과 일치해야 함. 신규 test 가 검증.

## 후속

- core-backlog 에 fast-follow 완료 표기 (PR 머지 후).
- 이슈 #82 에 P84 머지 안내 comment 추가 — 사용자가 명령 실행해 정리할 수 있도록 가이드.
