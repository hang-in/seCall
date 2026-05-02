---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 03
parallel_group: B
depends_on: [00]
---

# Task 03 — DB 스키마 v5 마이그레이션

## Changed files

수정:
- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION = 5`로 상향
- `crates/secall-core/src/store/schema.rs:3-25` — `CREATE_SESSIONS`에 `is_favorite INTEGER DEFAULT 0` 컬럼 추가 (신규 DB용)
- `crates/secall-core/src/store/db.rs:60-95` — `current < 5` 마이그레이션 분기 추가 (`ALTER TABLE sessions ADD COLUMN is_favorite`)
- `crates/secall-core/src/store/session_repo.rs:54-77` — `insert_session`에서 빈 태그 배열 대신 빈 배열 그대로 (혹은 정규화 규칙 통과). `is_favorite`는 DEFAULT 0이므로 INSERT 컬럼 목록에 명시 안 함 (이전 버전 컬럼 명시와 호환)

신규:
- `crates/secall-core/src/store/tag_normalize.rs` — 태그 정규화 유틸 (Task 03이 import해서 사용)
- `crates/secall-core/src/store/mod.rs` — `pub mod tag_normalize; pub use tag_normalize::normalize_tag;` 추가

## Change description

### 1. 스키마 버전 상향

`crates/secall-core/src/store/schema.rs:1`:
```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 5;
```

### 2. `CREATE_SESSIONS` 신규 컬럼

신규 DB에서 처음부터 `is_favorite` 갖도록:
```rust
pub const CREATE_SESSIONS: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    agent       TEXT NOT NULL,
    model       TEXT,
    project     TEXT,
    cwd         TEXT,
    git_branch  TEXT,
    start_time  TEXT NOT NULL,
    end_time    TEXT,
    turn_count  INTEGER DEFAULT 0,
    tokens_in   INTEGER DEFAULT 0,
    tokens_out  INTEGER DEFAULT 0,
    tools_used  TEXT,
    tags        TEXT,
    vault_path    TEXT,
    host          TEXT,
    summary       TEXT,
    ingested_at   TEXT NOT NULL,
    status        TEXT DEFAULT 'raw',
    session_type  TEXT DEFAULT 'interactive',
    is_favorite   INTEGER DEFAULT 0
);
";
```

### 3. 마이그레이션 분기 추가

`crates/secall-core/src/store/db.rs:60-95`의 마이그레이션 함수에 v5 분기:
```rust
if current < 5 && !self.column_exists("sessions", "is_favorite")? {
    self.conn.execute(
        "ALTER TABLE sessions ADD COLUMN is_favorite INTEGER DEFAULT 0",
        [],
    )?;
}
```

기존 v4 분기 다음에 위치. `column_exists` 헬퍼는 이미 존재 (`db.rs:97`).

### 4. 인덱스 (선택, 즐겨찾기 필터 성능)

`CREATE_INDEXES`에 추가:
```sql
CREATE INDEX IF NOT EXISTS idx_sessions_favorite ON sessions(is_favorite) WHERE is_favorite = 1;
```

> 부분 인덱스 — 즐겨찾기는 소수일 가능성 높음. SQLite 3.8+ 지원. rusqlite 0.31은 충분.

### 5. 태그 정규화 유틸 분리

`crates/secall-core/src/store/tag_normalize.rs` 신규:
```rust
//! 태그 정규화 — Task 03 (rest)와 insert 경로에서 공유.

const MAX_TAG_LEN: usize = 32;

pub fn normalize_tag(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    let replaced: String = lower
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    replaced.chars().take(MAX_TAG_LEN).collect()
}

pub fn normalize_tags(raw: &[String]) -> Vec<String> {
    raw.iter()
        .map(|s| normalize_tag(s))
        .filter(|s| !s.is_empty())
        .collect::<std::collections::BTreeSet<_>>()  // 중복 제거 + 정렬
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase() {
        assert_eq!(normalize_tag("Rust"), "rust");
    }

    #[test]
    fn whitespace_to_dash() {
        assert_eq!(normalize_tag("hello world"), "hello-world");
    }

    #[test]
    fn truncates_to_32() {
        let long = "a".repeat(50);
        assert_eq!(normalize_tag(&long).len(), 32);
    }

    #[test]
    fn strips_illegal_chars() {
        assert_eq!(normalize_tag("rust!@#$"), "rust");
    }

    #[test]
    fn deduplicates() {
        let tags = vec!["rust".into(), "Rust".into(), "RUST".into()];
        assert_eq!(normalize_tags(&tags), vec!["rust"]);
    }
}
```

`crates/secall-core/src/store/mod.rs`에 추가:
```rust
pub mod tag_normalize;
pub use tag_normalize::{normalize_tag, normalize_tags};
```

> Task 03이 `use crate::store::normalize_tags;` 로 사용.

### 6. `insert_session`은 변경 불필요

기존 `INSERT OR IGNORE INTO sessions(...)` 컬럼 목록에 `is_favorite` 미포함. SQLite는 미명시 컬럼에 DEFAULT 적용 → `is_favorite = 0`. 신규 세션은 자동으로 즐겨찾기 아님.

> 향후 `tags` 정규화도 insert 경로에서 적용하려면 `crates/secall-core/src/store/session_repo.rs:70`의 `serde_json::to_string(&Vec::<String>::new())`을 그대로 두되, P33 또는 별도 task에서 다룸. Task 04 범위는 컬럼 추가 + 정규화 유틸 분리만.

## Dependencies

- Task 01 완료 (워크스페이스 빌드)

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core --all-features

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 정규화 테스트
cargo test -p secall-core --lib store::tag_normalize

# 4. 전체 테스트 회귀
cargo test --all

# 5. 마이그레이션 검증 — v4 DB에 v5 적용
# 임시 디렉토리 + sqlite로 v4 스키마 만들고 Database::open으로 v5 마이그레이션 적용
mkdir -p /tmp/secall-mig-test
cat > /tmp/secall-mig-test.sh <<'SH'
set -euo pipefail
TEST_DIR=$(mktemp -d)
DB="$TEST_DIR/test.db"
sqlite3 "$DB" <<SQL
CREATE TABLE sessions (id TEXT PRIMARY KEY, agent TEXT NOT NULL, model TEXT, project TEXT, cwd TEXT, git_branch TEXT, start_time TEXT NOT NULL, end_time TEXT, turn_count INTEGER DEFAULT 0, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0, tools_used TEXT, tags TEXT, vault_path TEXT, host TEXT, summary TEXT, ingested_at TEXT NOT NULL, status TEXT DEFAULT 'raw', session_type TEXT DEFAULT 'interactive');
CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
INSERT INTO config(key, value) VALUES ('schema_version', '4');
INSERT INTO sessions(id, agent, start_time, ingested_at) VALUES ('test1', 'claude-code', '2026-05-01T00:00:00Z', '2026-05-02T00:00:00Z');
SQL

# Database::open이 마이그레이션 수행하도록 작은 Rust 검증
cargo run --quiet --bin secall -- status --db "$DB" || true  # status 명령 호출로 open trigger
sqlite3 "$DB" "SELECT name FROM pragma_table_info('sessions') WHERE name='is_favorite';"
sqlite3 "$DB" "SELECT value FROM config WHERE key='schema_version';"
SH
bash /tmp/secall-mig-test.sh
# 기대: is_favorite 출력, schema_version=5

# 6. # Manual: 위 스크립트의 마지막 두 sqlite3 출력에서
#   - "is_favorite" 한 줄
#   - "5" 한 줄
#   둘 다 보여야 마이그레이션 성공
```

> 위 라이브 검증이 secall CLI에 `status --db` 옵션이 없으면 작은 통합 테스트로 대체:
> `crates/secall-core/tests/migration_v5.rs` 작성 (rusqlite로 v4 schema 만들고 `Database::open`로 v5 적용 검증).

## Risks

- **마이그레이션 부분 실패**: ALTER TABLE 실패 시 schema_version 미상향. 다음 실행 시 재시도 가능하지만 기존 코드와 일관됨
- **부분 인덱스 호환성**: SQLite 3.8.0 이상에서만 부분 인덱스 지원. rusqlite-bundled 0.31의 SQLite 버전은 충분 (3.45+) — 안전
- **기존 DB의 `tags` 컬럼은 그대로 빈 배열**: insert_session이 `serde_json::to_string(&Vec::<String>::new())`을 넣으므로 NULL 아님. UPDATE만 가능. 정규화 적용 안 됨 — Task 03이 PATCH로 정규화 후 재저장
- **session_repo.rs:54-77 INSERT 컬럼 목록 유지**: `is_favorite` 미명시 → DEFAULT 사용. 만약 향후 명시 INSERT 필요 시 컬럼 추가
- **`SessionListItem`의 `is_favorite` 매핑 (Task 03)**: SELECT 시 `is_favorite` 컬럼 NULL이 아닌 0/1 정수 보장 필요. v5 마이그레이션 직후 기존 row의 `is_favorite`은 NULL일 수 있음 (ADD COLUMN DEFAULT는 SQLite에서 새 row만 적용되는 경우 있음, 기존 row는 NULL). 검증 필요 — NULL이면 `UPDATE sessions SET is_favorite = 0 WHERE is_favorite IS NULL` 추가

> ⚠ **중요**: SQLite의 `ALTER TABLE ADD COLUMN ... DEFAULT 0`은 기존 row에 0을 채워줌 (3.31+). 안전하지만 v5 마이그레이션에서 명시적으로 `UPDATE sessions SET is_favorite = 0 WHERE is_favorite IS NULL` 추가하는 것이 더 안전 (방어적 코딩).

## Scope boundary

수정 금지:
- `crates/secall-core/src/mcp/` — Task 02, 03
- `crates/secall-core/src/web/` — Task 02
- `web/` — Task 05~08
- `.github/workflows/`, `README.md` — Task 09
- 기존 마이그레이션 분기 (current < 4 등) — 내용 변경 금지, 추가만 허용
