---
type: task
status: draft
plan: secall-refactor-p4
task_number: 2
title: "Database Repository 패턴"
parallel_group: B
depends_on: [1]
updated_at: 2026-04-06
---

# Task 02: Database Repository 패턴

## 문제

`Database` 구조체에 대한 `impl` 블록이 3개 파일에 분산되어 총 33개 public 메서드가 하나의 타입에 집중된다:

| 파일 | 시작 라인 | 메서드 수 | 역할 |
|---|---|---|---|
| `store/db.rs:15` | 15 | 23개 | 핵심 DB 관리 + 통계 + 마이그레이션 |
| `search/bm25.rs:184` | 184 | 8개 | 세션/턴 CRUD + FTS 검색 |
| `search/vector.rs:251` | 251 | 3개 | 벡터 테이블 관리 |

전체 API 파악이 어렵고, 새 기여자의 진입 장벽이 높다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/store/mod.rs` | 수정 | trait 정의 모듈 추가 |
| `crates/secall-core/src/store/db.rs` | 수정 | 핵심 메서드 유지 + trait impl |
| `crates/secall-core/src/store/session_repo.rs` | 신규 | `SessionRepo` trait 정의 + impl |
| `crates/secall-core/src/store/search_repo.rs` | 신규 | `SearchRepo` trait 정의 + impl |
| `crates/secall-core/src/store/vector_repo.rs` | 신규 | `VectorRepo` trait 정의 + impl |
| `crates/secall-core/src/search/bm25.rs:184+` | 수정 | `impl Database` 블록 → `SessionRepo`/`SearchRepo` 사용으로 전환 |
| `crates/secall-core/src/search/vector.rs:251+` | 수정 | `impl Database` 블록 → `VectorRepo` 사용으로 전환 |

## Change description

### Step 1: trait 정의

```rust
// store/session_repo.rs — 신규
use crate::error::Result;
use crate::ingest::{Session, Turn};
use super::db::SessionMeta;

pub trait SessionRepo {
    fn insert_session(&self, session: &Session) -> Result<()>;
    fn update_session_vault_path(&self, session_id: &str, vault_path: &str) -> Result<()>;
    fn insert_turn(&self, session_id: &str, turn: &Turn) -> Result<i64>;
    fn session_exists(&self, session_id: &str) -> Result<bool>;
    fn session_exists_by_prefix(&self, prefix: &str) -> Result<bool>;
    fn get_session_meta(&self, session_id: &str) -> Result<SessionMeta>;
}

// store/search_repo.rs — 신규
pub trait SearchRepo {
    fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_index: u32) -> Result<()>;
    fn search_fts(&self, tokenized_query: &str, limit: usize, filters: &SearchFilters) -> Result<Vec<FtsRow>>;
}

// store/vector_repo.rs — 신규
pub trait VectorRepo {
    fn init_vector_table(&self) -> Result<()>;
    fn insert_vector(&self, embedding: &[f32], session_id: &str, turn_index: u32, chunk_seq: u32, model: &str) -> Result<i64>;
    fn search_vectors(&self, query_embedding: &[f32], limit: usize, session_ids: Option<&[String]>) -> Result<Vec<VectorRow>>;
}
```

### Step 2: 기존 impl Database 블록을 trait impl로 전환

```rust
// bm25.rs:184 — 변경 전
impl Database {
    pub fn insert_session(&self, session: &Session) -> Result<()> { ... }
    pub fn search_fts(...) -> Result<Vec<FtsRow>> { ... }
    // ... 8개 메서드
}

// 변경 후
impl SessionRepo for Database {
    fn insert_session(&self, session: &Session) -> Result<()> { ... }
    fn session_exists(&self, session_id: &str) -> Result<bool> { ... }
    fn get_session_meta(&self, session_id: &str) -> Result<SessionMeta> { ... }
    // ... 6개 메서드
}

impl SearchRepo for Database {
    fn insert_fts(...) -> Result<()> { ... }
    fn search_fts(...) -> Result<Vec<FtsRow>> { ... }
}
```

```rust
// vector.rs:251 — 변경 전
impl Database {
    pub fn init_vector_table(&self) -> Result<()> { ... }
    pub fn insert_vector(...) -> Result<i64> { ... }
    pub fn search_vectors(...) -> Result<Vec<VectorRow>> { ... }
}

// 변경 후
impl VectorRepo for Database {
    fn init_vector_table(&self) -> Result<()> { ... }
    fn insert_vector(...) -> Result<i64> { ... }
    fn search_vectors(...) -> Result<Vec<VectorRow>> { ... }
}
```

### Step 3: 호출자에 trait import 추가

trait 메서드를 호출하는 파일에 `use` 추가:

```rust
// search/hybrid.rs (또는 사용하는 위치)
use crate::store::{SessionRepo, SearchRepo, VectorRepo};
```

> Rust의 trait 메서드 호출 규칙상, trait이 스코프에 import되어야 메서드 접근 가능.

### Step 4: db.rs의 핵심 메서드 유지

`db.rs:15`의 `impl Database` 블록은 유지한다. 이 블록의 메서드는 DB 인프라 관련 (open, migrate, conn, with_transaction, get_stats 등)이므로 trait 분리 대상이 아님.

통계/조회 메서드 (`count_sessions`, `list_projects`, `list_agents`, `has_embeddings` 등)는 `StatsRepo` trait으로 분리하거나 `impl Database`에 유지. 1차에서는 유지하고, 메서드가 10개를 넘으면 후속 분리.

### Step 5: 기존 호환성 유지

trait의 메서드 시그니처를 기존 `pub fn`과 동일하게 유지한다. Rust에서 trait impl의 메서드는 trait이 스코프에 있으면 기존과 동일하게 `db.insert_session(...)` 형태로 호출 가능.

## Dependencies

- **Task 01 (typed error)**: `Result` 타입이 `SecallError` 기반으로 변경되어야 trait 시그니처가 확정됨
- P3 CI 안전망 필수 (33개 메서드 이동은 회귀 위험 높음)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과 (33개 메서드 시그니처 호환 검증)
cargo test --all

# 3. trait이 public API에 노출되는지 확인
cargo doc -p secall-core --no-deps 2>&1 | grep -c "SessionRepo\|SearchRepo\|VectorRepo"

# 4. clippy 통과
cargo clippy --all-targets -- -D warnings

# 5. 기존 `impl Database` 블록이 bm25.rs/vector.rs에 남아있지 않은지 확인
grep -n "^impl Database" crates/secall-core/src/search/bm25.rs crates/secall-core/src/search/vector.rs
# 예상: 출력 없음 (모두 trait impl로 전환)
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요.

## Risks

- **대규모 이동**: 33개 메서드 중 11개(bm25 8개 + vector 3개)가 파일 간 이동. `impl Database` → `impl Trait for Database`로 변경 시 메서드 본문은 동일하나, 호출자에 trait import가 필요.
- **trait object 사용 시 제약**: `SearchRepo`를 `dyn SearchRepo`로 사용하려면 object safety가 필요. 현재 모든 메서드가 `&self`만 사용하므로 object-safe.
- **순환 의존**: `store/` 모듈에서 `search/bm25.rs`의 타입(`FtsRow`, `SearchFilters`)을 참조해야 할 수 있다. 이 경우 타입 정의를 `store/` 또는 공통 모듈로 이동 필요.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/store/schema.rs` — DDL 변경 없음
- `crates/secall-core/src/mcp/server.rs` — MCP 서버는 `Database` 타입을 직접 사용하므로 trait import만 추가. 로직 변경 없음.
- `crates/secall/src/commands/*.rs` — bin crate는 `Database` 타입을 직접 사용. trait import 추가만 허용.
- `crates/secall-core/src/vault/` — Vault는 DB를 사용하지 않으므로 영향 없음
