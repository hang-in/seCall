---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 01
title: DB migration (schema v10) — is_archived / archived_at + partial index
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-12
---

# Task 01 — DB migration (schema v10)

## Changed files

수정:

- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION` 9 → 10.
- `crates/secall-core/src/store/schema.rs:4-27` 의 `CREATE_SESSIONS` 상수 — 신규 vault 에 처음 생성될 때를 위해 `is_archived INTEGER NOT NULL DEFAULT 0` 와 `archived_at TEXT NULL` 두 컬럼을 정의 끝에 추가.
- `crates/secall-core/src/store/db.rs:60-130` 의 migration 분기 — 기존 `if current < 9` 블록 뒤에 `if current < 10 { ... }` 신규 분기 추가. 내부에서:
  1. `column_exists("sessions", "is_archived")` false 면 `ALTER TABLE sessions ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0`.
  2. `column_exists("sessions", "archived_at")` false 면 `ALTER TABLE sessions ADD COLUMN archived_at TEXT`.
  3. 방어적 보정: `UPDATE sessions SET is_archived = 0 WHERE is_archived IS NULL`.
  4. partial index 생성: `CREATE INDEX IF NOT EXISTS idx_sessions_archived ON sessions(is_archived) WHERE is_archived = 1` — archived 비율이 낮을 것이므로 1쪽만 인덱싱.

신규:

- 없음.

회귀 테스트:

- 기존 schema 회귀 (`db.rs` 안의 `#[cfg(test)] mod tests`) 에 `test_migration_to_v10_adds_archive_columns` 추가 — v9 schema 로 만든 임시 DB 에 v10 migration 을 돌려 `column_exists` 가 두 컬럼 모두 true 반환하는지 검증.

## Change description

### 1. schema.rs 갱신

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 10;
```

`CREATE_SESSIONS` 마지막에 두 컬럼 추가:

```sql
    -- ... 기존 컬럼들 ...
    semantic_extracted_at INTEGER,
    is_archived           INTEGER NOT NULL DEFAULT 0,
    archived_at           TEXT
```

> 신규 vault 에서 처음 생성될 때 컬럼이 있도록 함. 기존 vault 는 ALTER TABLE 분기로 처리.

### 2. db.rs migration 분기 추가

기존 (`db.rs:120-125`):

```rust
        if current < 9 {
            self.conn.execute_batch(CREATE_WIKI_VECTORS)?;
        }
```

뒤에 추가:

```rust
        if current < 10 {
            if !self.column_exists("sessions", "is_archived")? {
                self.conn.execute(
                    "ALTER TABLE sessions ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0",
                    [],
                )?;
            }
            if !self.column_exists("sessions", "archived_at")? {
                self.conn
                    .execute("ALTER TABLE sessions ADD COLUMN archived_at TEXT", [])?;
            }
            // 방어적 보정 — ALTER TABLE ADD COLUMN DEFAULT 가 기존 row 에 적용 안 된 경우
            self.conn.execute(
                "UPDATE sessions SET is_archived = 0 WHERE is_archived IS NULL",
                [],
            )?;
            self.conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_sessions_archived \
                 ON sessions(is_archived) WHERE is_archived = 1;",
            )?;
        }
```

> 패턴은 기존 `is_favorite` (v5, db.rs:91-104) 와 동일.

### 3. 인덱스 선택 — `WHERE is_archived = 1`

archived 세션은 소수일 것이므로 partial index 를 1쪽에만 둔다. 기본 검색 (`is_archived = 0`) 은 full-table 가정이지만 sessions 테이블 자체가 크지 않아 성능 영향 미미. 0쪽 인덱스는 row 대부분을 포함해 비효율적이므로 만들지 않는다.

(추후 archived 비율이 50% 가까워지면 인덱스 전략 재검토.)

## Dependencies

- 의존 task 없음 (parallel_group A 시작).
- crate dep: 추가 없음.

## Verification

```bash
# 1. type check
cargo check -p secall-core

# 2. schema 회귀 + 신규 migration 테스트
cargo test -p secall-core --lib store::db::tests::test_migration_to_v10
cargo test -p secall-core --lib store::db::tests

# 3. 기존 schema 회귀 ( 인덱스 충돌 / 컬럼 누락 없는지 )
cargo test -p secall-core --lib store::

# 4. SQL 직접 확인 (수동) — 임시 DB 생성 후 schema 확인
# cargo run -p secall -- init --vault /tmp/p45-test
# sqlite3 /tmp/p45-test/.secall/db.sqlite ".schema sessions" | grep -E "is_archived|archived_at"
```

## Risks

- **기존 DB 의 row UPDATE 비용** — `UPDATE sessions SET is_archived = 0` 가 모든 row 를 스캔. 사용자 vault 의 세션 수가 수만 건이면 migration 한 번에 수 초. 일회성이므로 허용.
- **`NOT NULL DEFAULT 0` 와 ALTER TABLE** — SQLite 는 ADD COLUMN 시 DEFAULT 가 기존 row 에 즉시 적용됨 (3.35+). 그러나 일부 환경에서 NULL 잔존 가능성 → UPDATE 보정으로 안전.
- **partial index 호환성** — SQLite 3.8+ 지원. seCall 의 bundled rusqlite (rusqlite default features) 는 충분히 신버전. 호환 OK.
- **schema version 충돌** — 동시 plan 이 v10 을 다른 용도로 사용할 가능성 없음 (현재 진행 중인 plan 은 P45 뿐).
- **마이그레이션 idempotent 깨질 위험** — `column_exists` 가드로 보호되지만, 향후 v11 추가 시 v10 분기는 그대로 두어 신규 vault 가 v0 → v10 → v11 순차 적용되도록 유지.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/session_repo.rs` — task 04 / 05 영역.
- `crates/secall-core/src/vault/mod.rs` — task 02 영역.
- `crates/secall-core/src/ingest/markdown.rs` — task 02 / 03 영역.
- `crates/secall-core/src/search/{bm25,hybrid,vector}.rs` — task 05 영역.
- `crates/secall-core/src/mcp/server.rs` — task 05 영역.
- `crates/secall/src/commands/*` — REST / CLI 는 본 plan 영역 외 (P46).
