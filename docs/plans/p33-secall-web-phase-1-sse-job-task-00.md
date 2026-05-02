---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — DB 스키마 v6 (`jobs` 테이블 + cleanup)

## Changed files

수정:
- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION = 6`
- `crates/secall-core/src/store/schema.rs` — `CREATE_JOBS` const 추가 (`pub const CREATE_JOBS: &str = "..."`)
- `crates/secall-core/src/store/db.rs:60-100` 부근 — v6 마이그레이션 분기 추가 + `import` 갱신
- `crates/secall-core/src/store/db.rs` (테스트 모듈) — v6 신규 테스트 2-3개

신규:
- `crates/secall-core/src/store/jobs_repo.rs` — `Database`에 `JobRow` insert/get/list/cleanup 메서드 추가

## Change description

### 1. 스키마 버전 상향

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 6;
```

### 2. `CREATE_JOBS` 정의

```rust
pub const CREATE_JOBS: &str = "
CREATE TABLE IF NOT EXISTS jobs (
    id            TEXT PRIMARY KEY,        -- UUID v4
    kind          TEXT NOT NULL,           -- 'sync' | 'ingest' | 'wiki_update'
    status        TEXT NOT NULL,           -- 'started' | 'running' | 'completed' | 'failed' | 'interrupted'
    started_at    TEXT NOT NULL,           -- RFC3339
    completed_at  TEXT,                    -- RFC3339, NULL while running
    error         TEXT,                    -- error message if failed
    result        TEXT,                    -- JSON: phase results, counts, etc.
    metadata      TEXT                     -- JSON: input args (e.g. {local_only: true})
);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_started_at ON jobs(started_at);
";
```

> `started`(전: 큐 들어감, 실제 실행 안 시작) vs `running`(실행 중) 구분. Job executor가 spawn 직후 → `started`, phase 1 진입 시 → `running`으로 갱신.

### 3. 마이그레이션 분기

`crates/secall-core/src/store/db.rs`의 `migrate()` 함수에서 v5 분기 다음에:

```rust
if current < 6 {
    self.conn.execute_batch(CREATE_JOBS)?;
    // 시작 시 1회 cleanup: 7일 이상된 완료/실패/중단 jobs 삭제
    self.conn.execute(
        "DELETE FROM jobs WHERE completed_at IS NOT NULL AND completed_at < datetime('now', '-7 days')",
        [],
    )?;
}
```

> 신규 DB라면 `CREATE_JOBS`만 실행. 기존 v5 DB는 ALTER 불필요 (테이블 신규 생성).

### 4. `jobs_repo.rs` — `Database` 메서드

```rust
pub struct JobRow {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

impl Database {
    /// Job 시작 시 INSERT. `started` 상태로 기록.
    pub fn insert_job(&self, id: &str, kind: &str, metadata: Option<&serde_json::Value>) -> Result<()>;

    /// Job 상태 갱신 (메모리 → DB는 완료/실패/중단 시점에만).
    pub fn complete_job(&self, id: &str, status: &str, result: Option<&serde_json::Value>, error: Option<&str>) -> Result<()>;

    /// 단일 Job 조회 (재접속 시 진행 상태 fallback 또는 결과 영구 조회).
    pub fn get_job(&self, id: &str) -> Result<Option<JobRow>>;

    /// 최근 N개 jobs (관리 UI용).
    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobRow>>;

    /// 시작 시 1회 cleanup. 7일 이상된 완료/실패/중단 jobs 삭제.
    /// 반환값: 삭제된 row 수.
    pub fn cleanup_old_jobs(&self) -> Result<usize>;
}
```

> 진행 중 (`running`) job은 메모리에만 있음. 서버 재시작 시 메모리 손실 → 시작 시 `running`/`started` 상태 jobs를 모두 `interrupted`로 일괄 갱신하는 것도 옵션 (Task 02에서 결정).

### 5. `store/mod.rs` 등록

```rust
pub mod jobs_repo;
pub use jobs_repo::JobRow;
```

### 6. 테스트

`crates/secall-core/src/store/db.rs` tests 모듈에:
- `test_v6_jobs_table_exists` — 신규 DB에 jobs 테이블 생성 확인
- `test_v6_migrates_v5_db` — v5 DB에 jobs 테이블 추가 마이그레이션 확인
- `test_jobs_insert_and_complete` — insert → complete 플로우 검증
- `test_cleanup_old_jobs` — 7일 이상된 row만 삭제

## Dependencies

- 외부: 없음 (rusqlite 0.31 이미 있음)
- 내부 task: 없음 (root)

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core --all-features

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 v6 테스트
cargo test -p secall-core --lib store::db::tests::test_v6
cargo test -p secall-core --lib store::db::tests::test_jobs
cargo test -p secall-core --lib store::db::tests::test_cleanup

# 4. 전체 테스트 회귀
cargo test --all
```

## Risks

- **schema_version 5 → 6 마이그레이션**: v5 DB에 jobs 테이블만 추가. ALTER 불필요. 기존 데이터 영향 없음
- **시작 시 running jobs 처리**: 본 task는 schema만 다룸. "재시작 시 running → interrupted 일괄 갱신"은 Task 02 (Job registry) 책임으로 분리
- **JSON 컬럼 (`result`, `metadata`)**: TEXT로 저장. 응답 직렬화 시 `serde_json::from_str` 실패 가능 — `Option<Value>` 폴백 필요
- **인덱스**: `idx_jobs_status`로 "현재 실행 중인 jobs 조회"가 빠름. `idx_jobs_started_at`은 cleanup 효율
- **datetime('now', '-7 days')**: SQLite 표준 함수. 타임존 영향은 UTC 기준이라 안전

## Scope boundary

수정 금지:
- `crates/secall-core/src/jobs/` — Task 02 (Job 코어 모듈)
- `crates/secall-core/src/mcp/` — Task 03, 04
- `crates/secall/src/commands/` — Task 03, 08
- `web/` — Task 05, 06, 07
- `.github/workflows/`, `README*` — Task 09
- 기존 v1~v5 마이그레이션 분기 — 내용 변경 금지, v6 분기 추가만 허용
