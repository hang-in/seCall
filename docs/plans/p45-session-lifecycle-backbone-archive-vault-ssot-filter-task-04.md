---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 04
title: Store archive_session / restore_session — DB + vault frontmatter 트랜잭션
parallel_group: B
depends_on: [01, 02]
status: pending
updated_at: 2026-05-12
---

# Task 04 — Store archive_session / restore_session

## Changed files

수정:

- `crates/secall-core/src/store/session_repo.rs` — `impl SessionRepo<'_>` (또는 동등 위치) 에 두 메서드 추가:
  - `pub fn archive_session(&self, session_id: &str, vault: &Vault) -> Result<()>`
  - `pub fn restore_session(&self, session_id: &str, vault: &Vault) -> Result<()>`
- `crates/secall-core/src/lib.rs` — 필요 시 `pub use` re-export 확인 (`SessionRepo`, `Vault` 둘 다 이미 노출되어 있어야 함).

신규:

- 없음.

회귀 테스트:

- `crates/secall-core/src/store/session_repo.rs` 의 `#[cfg(test)] mod tests` (또는 `tests/session_archive_unit.rs` 신규):
  1. `test_archive_session_sets_db_and_frontmatter` — 임시 vault + DB 에 세션 1개 만들고 archive_session 호출 → DB 의 `is_archived=1`, vault 파일의 frontmatter 에 `archived: true` 두 라인 존재.
  2. `test_restore_session_clears_db_and_frontmatter` — 1번 결과를 restore → DB `is_archived=0`, vault frontmatter 에서 두 라인 제거.
  3. `test_archive_session_vault_write_fails_rolls_back_db` — vault path 가 존재하지 않는 (또는 read-only) 케이스 simulate → DB 도 변경되지 않음 (rollback).
  4. `test_archive_session_unknown_id_returns_error` — 없는 session_id 호출 시 NotFound 명확한 에러.

## Change description

### 1. archive_session — DB + vault 트랜잭션

```rust
impl<'a> SessionRepo<'a> {
    /// 세션을 archive — DB row 업데이트 + vault frontmatter 갱신.
    /// vault write 실패 시 DB rollback. caller 는 vault 인스턴스 전달.
    pub fn archive_session(&self, session_id: &str, vault: &Vault) -> Result<()> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        // 1) 세션 존재 + vault_path 조회
        let (vault_path, current_archived): (Option<String>, i64) = self
            .conn()
            .query_row(
                "SELECT vault_path, is_archived FROM sessions WHERE id = ?1",
                rusqlite::params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => crate::SecallError::Config(format!(
                    "session not found: {session_id}"
                )),
                other => other.into(),
            })?;

        if current_archived == 1 {
            // idempotent — 이미 archived 면 no-op (vault frontmatter 도 동일 가정)
            return Ok(());
        }

        // 2) BEGIN 트랜잭션
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE sessions SET is_archived = 1, archived_at = ?1 WHERE id = ?2",
            rusqlite::params![now_str, session_id],
        )?;

        // 3) vault frontmatter update — 실패 시 트랜잭션 rollback
        if let Some(rel) = &vault_path {
            vault
                .update_session_archive_frontmatter(rel, true, Some(now), tz_from_config())
                .map_err(|e| {
                    // tx 는 drop 시 자동 rollback (unchecked_transaction)
                    crate::SecallError::Config(format!(
                        "vault frontmatter update failed for {session_id}: {e}"
                    ))
                })?;
        }
        // vault_path 가 None 이면 (구버전 row) DB 만 업데이트하고 경고 — 일단 silent OK

        // 4) commit
        tx.commit()?;
        Ok(())
    }

    pub fn restore_session(&self, session_id: &str, vault: &Vault) -> Result<()> {
        // archive_session 의 대칭 구현 — is_archived=0, archived_at=NULL.
        // vault frontmatter 의 두 라인 제거.
        // 상세 구현 동일 패턴.
    }
}
```

> `unchecked_transaction` 은 rusqlite 의 transaction guard. Drop 시 rollback, commit 명시.
> `tz_from_config()` 는 caller 가 timezone 을 알아야 하므로 시그니처에 `tz: chrono_tz::Tz` 인자 추가 — 또는 caller 가 `Config::load_or_default().timezone()` 전달.

### 2. 시그니처 변형 — 명시적 timezone

caller side 부담을 줄이려면:

```rust
pub fn archive_session(
    &self,
    session_id: &str,
    vault: &Vault,
    tz: chrono_tz::Tz,
) -> Result<()> { ... }
```

REST / CLI / 테스트 모두 `config.timezone()` 또는 `chrono_tz::UTC` 명시. 테스트는 UTC 사용으로 결정 단순화.

### 3. idempotent / error 정책

- 이미 archived 인 세션 archive → no-op (silent OK).
- 이미 restored (is_archived=0) 인 세션 restore → no-op.
- 없는 session_id → `SecallError::Config("session not found: ...")` 또는 별도 `SessionNotFound` variant (별도 plan).
- vault_path 가 NULL (예: 구버전 row) → DB 만 업데이트, warning 로깅 (eprintln!), Ok 반환.
- vault frontmatter parse 실패 → rollback + error 전파.

### 4. transaction 의 일관성

- `rusqlite::Connection` 은 단일 connection 가정.
- `unchecked_transaction` 은 nested transaction 시 SAVEPOINT 가 아닌 단일 transaction — 다른 곳에서 이미 tx 사용 중이면 conflict. seCall 의 REST/CLI 진입에서 tx 중첩이 없는 것을 caller 가 보장 (현재 호출자는 task 본 plan 외이므로 caller 측 검증은 P46 영역).

## Dependencies

- task 01 — `is_archived` / `archived_at` 컬럼이 schema 에 존재해야 함.
- task 02 — `Vault::update_session_archive_frontmatter` helper 가 존재해야 함.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. archive/restore 단위 테스트
cargo test -p secall-core --lib store::session_repo::tests::test_archive_session
cargo test -p secall-core --lib store::session_repo::tests::test_restore_session
cargo test -p secall-core --lib store::session_repo::tests::test_archive_session_vault_write_fails_rolls_back_db
cargo test -p secall-core --lib store::session_repo::tests::test_archive_session_unknown_id_returns_error

# 3. SessionRepo 기존 회귀
cargo test -p secall-core --lib store::session_repo
```

## Risks

- **vault_path 가 NULL 인 구버전 row** — DB 만 archive 처리 후 vault 와 sync 안 됨. 다음 sync 의 re-ingest 시 frontmatter 에 archived 없으므로 다시 0 으로 되돌려짐. mitigation: vault_path 가 NULL 인 row 에 대해 archive 시 명시적 에러 또는 vault 파일을 찾는 fallback (filename → session_id prefix). 본 task 에선 silent + warn 만 (단순성 우선).
- **transaction 중 다른 ingest 가 vault 파일을 덮어쓸 위험** — 사용자가 두 머신에서 동시에 같은 세션 archive 시 race. seCall 단일 사용자 + 단일 machine session lifecycle 가정. cross-host 충돌은 git merge 로 후처리 (P44 패턴).
- **vault update 후 commit 실패** — DB rollback 되지만 vault 파일은 이미 변경된 상태. 다음 ingest 가 vault 를 SSOT 로 DB 재동기화 → 최종 일관성 OK. 단 한 순간 vault=archived/DB=not archived 의 inconsistency 존재. mitigation: vault update 를 commit 직후로 옮기는 옵션도 있으나, vault 실패 → DB rollback 패턴이 사용자 의도에 더 부합 (vault SSOT).
- **`unchecked_transaction` 의 nested tx 위험** — 호출자가 이미 tx 안에 있으면 panic 또는 SQL error. 본 task 의 메서드는 self-contained tx 라 caller 가 외부 tx 만들지 말 것을 문서화.
- **시간대 일관성** — `archived_at` 의 timezone offset 이 머신마다 다르면 frontmatter 비교 어려움. mitigation: 항상 UTC 저장 + tz 는 표시용만. 본 task 의 `Utc::now().to_rfc3339()` 는 UTC suffix `Z` 포함.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/schema.rs` — task 01 영역.
- `crates/secall-core/src/vault/mod.rs::update_session_archive_frontmatter` — task 02 영역 (호출만).
- `crates/secall-core/src/ingest/markdown.rs` — task 02 / 03 영역.
- `crates/secall-core/src/store/session_repo.rs::insert_session_from_vault` — task 03 영역.
- `crates/secall-core/src/search/*` / `crates/secall-core/src/mcp/server.rs` — task 05 영역.
- `crates/secall/src/commands/*` — REST/CLI 는 P46.
