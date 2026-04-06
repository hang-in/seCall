# Review Report: seCall P5 — Vault Git Sync + Reindex — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 18:21
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/sync.rs:13 — `run(local_only, _dry_run)`에서 `dry_run` 인자를 버리고 실제 pull/reindex/ingest/push를 그대로 수행합니다. Task 03 문서의 `--dry-run` 계약과 달리 `secall sync --dry-run`이 상태를 변경하므로 기능 결함입니다.
2. crates/secall-core/src/vault/git.rs:24 — `init()`은 단순 `git init`만 실행해 로컬 기본 브랜치를 Git 환경설정에 맡기는데, 같은 모듈의 `pull()`/`push()`는 `origin main`을 하드코딩합니다. 기본 브랜치가 `main`이 아닌 환경에서는 방금 초기화한 vault가 이후 sync에서 pull/push 실패합니다.
3. crates/secall-core/src/vault/git.rs:92 — `push()`는 `raw/`와 `wiki/`만 stage합니다. 그런데 vault 쓰기 경로는 [mod.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/vault/mod.rs#L36)에서 `index.md`와 `log.md`도 함께 갱신하므로, sync 후 원격에는 루트 메타데이터 변경이 누락되어 vault 상태가 일관되지 않게 됩니다.

## Recommendations

1. Task 03의 `dry_run`은 각 phase에서 실행 대신 예정 작업만 출력하고 조기 반환하도록 분기하세요.
2. Task 02는 초기화 시 브랜치를 명시적으로 `main`으로 만들거나, 현재 브랜치/설정된 remote 기본 브랜치를 읽어 pull/push에 재사용하는 쪽이 안전합니다.
3. `reindex` 로직이 [sync.rs](/Users/d9ng/privateProject/seCall/crates/secall/src/commands/sync.rs)와 [reindex.rs](/Users/d9ng/privateProject/seCall/crates/secall/src/commands/reindex.rs)에 중복되어 있어 후속 수정 시 불일치 위험이 큽니다. 공용 함수로 합치는 편이 낫습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | MD → DB 역인덱싱 (reindex --from-vault) | ✅ done |
| 2 | Vault Git 연동 (init/pull/push) | ✅ done |
| 3 | `secall sync` 통합 명령 | ✅ done |
| 4 | host 필드 추가 | ✅ done |

