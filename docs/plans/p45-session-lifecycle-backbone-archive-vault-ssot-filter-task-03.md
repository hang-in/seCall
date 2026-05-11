---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 03
title: Ingest frontmatter parser 확장 — archived / archived_at 읽어 DB UPSERT 시 반영
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-12
---

# Task 03 — Ingest frontmatter parser 확장

## Changed files

수정:

- `crates/secall-core/src/ingest/markdown.rs:9-26` 의 `SessionFrontmatter` struct — `pub archived: Option<bool>` 와 `pub archived_at: Option<String>` 두 필드 추가. 기본값은 `#[serde(default)]` 가 이미 struct-level 에 있으므로 None.
- `crates/secall-core/src/store/session_repo.rs:385-428` 의 `insert_session_from_vault` — INSERT 컬럼 리스트에 `is_archived`, `archived_at` 추가. params 에 `fm.archived.unwrap_or(false) as i64`, `fm.archived_at.clone()` 전달. `INSERT OR IGNORE` 는 기존 row 면 무시되므로 archive 상태 변경을 반영하려면 별도 UPDATE 가 필요 — 본 task 에서 처리 (아래 2번 항목).

신규:

- 없음.

회귀 테스트:

- `crates/secall-core/src/ingest/markdown.rs` 의 `#[cfg(test)] mod tests`:
  1. `test_parse_session_frontmatter_with_archived` — frontmatter 에 `archived: true / archived_at: "..."` 있을 때 SessionFrontmatter 의 두 필드가 올바르게 채워짐.
  2. `test_parse_session_frontmatter_without_archived_defaults_to_none` — 두 라인 없을 때 None.
- `crates/secall-core/src/store/session_repo.rs` 의 회귀 테스트 (또는 `tests/session_repo_helpers.rs`):
  3. `test_insert_session_from_vault_with_archived_sets_db` — archived=true frontmatter 로 insert 시 DB 의 `is_archived = 1` / `archived_at` 채워짐.
  4. `test_insert_session_from_vault_archived_changed_updates_db` — 기존 session row 가 있고 (is_archived=0), 같은 session_id 의 archived=true frontmatter 재insert 시 DB row 가 update 됨 (insert_session_from_vault 가 INSERT OR IGNORE 만 하지 말고 archive 변경분도 UPDATE 처리).

## Change description

### 1. SessionFrontmatter 필드 추가

```rust
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct SessionFrontmatter {
    pub session_id: String,
    // ... 기존 필드 ...
    pub session_type: Option<String>,

    /// P45 — vault 가 SSOT. archived=true 면 DB 의 is_archived 도 갱신.
    pub archived: Option<bool>,
    /// RFC3339 string. parser 가 read 만 함 (저장 시엔 markdown.rs 의 render_session 가 처리).
    pub archived_at: Option<String>,
}
```

`serde_yaml::from_str` 가 frontmatter YAML 을 그대로 디코딩 — 두 필드 추가만으로 자동 작동.

### 2. `insert_session_from_vault` 의 archive sync

기존 (`session_repo.rs:391-417`) 은 `INSERT OR IGNORE` 라 기존 row 있으면 무시. archived 상태 변경 반영을 위해 다음과 같이 분리:

```rust
pub fn insert_session_from_vault(
    &self,
    fm: &crate::ingest::markdown::SessionFrontmatter,
    body_text: &str,
    vault_path: &str,
) -> Result<()> {
    let archived_int: i64 = fm.archived.unwrap_or(false) as i64;
    let archived_at = fm.archived_at.clone();

    self.conn().execute(
        "INSERT OR IGNORE INTO sessions(
            id, agent, model, project, cwd, git_branch, host,
            start_time, end_time, turn_count, tokens_in, tokens_out,
            tools_used, vault_path, summary, ingested_at, status,
            is_archived, archived_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, NULL, ?6,
            ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, datetime('now'), 'reindexed',
            ?15, ?16
        )",
        rusqlite::params![
            fm.session_id,
            fm.agent,
            fm.model,
            fm.project,
            fm.cwd,
            fm.host,
            fm.start_time,
            fm.end_time,
            fm.turns.unwrap_or(0),
            fm.tokens_in.unwrap_or(0),
            fm.tokens_out.unwrap_or(0),
            fm.tools_used.as_ref().map(|t| t.join(",")),
            vault_path,
            fm.summary,
            archived_int,
            archived_at,
        ],
    )?;

    // P45 — 기존 row 가 있던 경우에도 vault frontmatter 의 archive 상태로 DB 동기화
    // (insert OR IGNORE 라 INSERT 가 무시됐을 수 있음). archive 상태만 단독 UPDATE.
    self.conn().execute(
        "UPDATE sessions SET is_archived = ?1, archived_at = ?2 WHERE id = ?3",
        rusqlite::params![archived_int, archived_at, fm.session_id],
    )?;

    // FTS 인덱싱 — 본문 전체를 하나의 청크로 (기존 그대로)
    if !body_text.trim().is_empty() {
        // ...
    }

    Ok(())
}
```

> 두 단계 (INSERT OR IGNORE + UPDATE archive) 로 분리. INSERT 가 성공하면 UPDATE 는 no-op. INSERT 가 무시됐으면 UPDATE 가 archive 상태만 sync.
> tags / favorite / notes 등은 본 task 범위 외 (DB only 유지).

### 3. ingest 진입 경로 확인

`crates/secall/src/commands/ingest.rs` 또는 `sync.rs` 의 vault re-scan 경로에서 `parse_session_frontmatter` → `insert_session_from_vault` 호출 체인 그대로 동작. 새 두 컬럼은 자동 흐름. 별도 변경 X.

(re-ingest 진입 자체는 P31 / P39 영역. 본 task 는 parser + DB sink 만.)

## Dependencies

- 의존 task 없음 (parallel_group A 시작).
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. parser 단위 테스트
cargo test -p secall-core --lib ingest::markdown::tests::test_parse_session_frontmatter_with_archived
cargo test -p secall-core --lib ingest::markdown::tests::test_parse_session_frontmatter_without_archived

# 3. insert_session_from_vault DB sync 테스트
cargo test -p secall-core --lib store::session_repo::tests::test_insert_session_from_vault_with_archived
cargo test -p secall-core --lib store::session_repo::tests::test_insert_session_from_vault_archived_changed_updates

# 4. 기존 ingest 회귀
cargo test -p secall-core --lib ingest::
cargo test -p secall-core --lib store::session_repo
```

## Risks

- **INSERT OR IGNORE + UPDATE 의 race condition** — 두 statement 사이 timing 동시성 문제. seCall 은 단일 connection / serial ingest 라 무영향.
- **archived_at 의 형식 검증 누락** — frontmatter 의 `archived_at` 가 잘못된 형식이면 그대로 DB 에 저장됨. mitigation: render 측 (task 02) 이 항상 정형 RFC3339 출력하므로 round-trip 안전. parser 측에서 chrono parse 검증은 영역 외.
- **UPDATE 가 항상 실행됨 — INSERT 가 성공한 경우 중복 비용** — 한 row 의 UPDATE 는 sub-ms. 수만 세션 re-ingest 시에도 누적 비용 무시 가능.
- **tags/favorite/notes 의 vault sync 미반영** — 본 task 영역 외 (P50+). DB only 동작 유지 — re-ingest 시 DB 의 tags 등은 vault frontmatter 와 무관하게 보존.
- **신규 vault row 에 archived 컬럼 SQL ERROR** — task 01 의 ALTER TABLE 이 선행돼야 함. task 01 / 03 모두 parallel_group A 지만 schema 변경이 binary 실행 시점에 먼저 일어남 (Database::open → migrate_schema). 충돌 없음.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/schema.rs` / `db.rs:60-130` — task 01 영역. 본 task 는 INSERT/UPDATE 쿼리만 변경.
- `crates/secall-core/src/vault/mod.rs` 의 `update_session_archive_frontmatter` — task 02 영역. 본 task 는 parser 만 확장.
- `crates/secall-core/src/store/session_repo.rs` 의 `list_sessions_filtered` / `list_sessions_for_graph_rebuild` — task 05 영역.
- `crates/secall-core/src/search/*` — task 05 영역.
- `crates/secall-core/src/mcp/server.rs` — task 05 영역.
- `crates/secall/src/commands/*` — REST/CLI 는 본 plan 영역 외 (P46).
