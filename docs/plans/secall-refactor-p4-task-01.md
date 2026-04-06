---
type: task
status: draft
plan: secall-refactor-p4
task_number: 1
title: "typed error 도입 (SecallError enum)"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: typed error 도입 (SecallError enum)

## 문제

`secall-core` crate의 모든 public 함수가 `anyhow::Result`를 반환한다. thiserror가 의존성에 있으나(`Cargo.toml:18`) 사용되지 않아, MCP 서버가 에러 종류를 구분할 수 없다.

### 현재 MCP 에러 매핑 (server.rs)

```rust
// server.rs:66-67 — DB lock 에러
self.db.lock().map_err(|e| McpError::internal_error(format!("DB lock error: {e}"), None))?;

// server.rs:71-73 — 검색 에러 (BM25 실패와 "결과 없음"이 구분 불가)
.map_err(|e| McpError::internal_error(format!("BM25 error: {e}"), None))?;

// server.rs:171-174 — not found (올바르게 invalid_params 사용)
Err(e) => McpError::invalid_params(format!("Turn not found: {e}"), None),
```

모든 에러가 `internal_error`로 매핑되어, 클라이언트가 복구 가능한 에러(입력 오류, 세션 미발견)와 시스템 에러(DB 장애)를 구분할 수 없다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/error.rs` | 신규 | `SecallError` enum 정의 |
| `crates/secall-core/src/lib.rs` | 수정 | `pub mod error;` 추가 + re-export |
| `crates/secall-core/src/store/db.rs` | 수정 | `open()`, `migrate()`, `with_transaction()` 등 핵심 메서드 반환 타입 변경 |
| `crates/secall-core/src/search/bm25.rs` | 수정 | `search_fts()`, `insert_session()` 등 반환 타입 변경 |
| `crates/secall-core/src/search/vector.rs` | 수정 | `search_vectors()`, `insert_vector()` 반환 타입 변경 |
| `crates/secall-core/src/search/hybrid.rs` | 수정 | `SearchEngine` 메서드 반환 타입 변경 |
| `crates/secall-core/src/ingest/mod.rs` | 수정 | `SessionParser::parse()` 반환 타입 변경 |
| `crates/secall-core/src/mcp/server.rs` | 수정 | `SecallError` variant 매칭으로 McpError 분기 |
| `crates/secall/src/commands/*.rs` | 수정 | CLI 명령에서 `SecallError` → 사용자 메시지 변환 |

## Change description

### Step 1: SecallError enum 정의 (error.rs — 신규)

```rust
// crates/secall-core/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecallError {
    // --- Store ---
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("database not initialized: run `secall init` first")]
    DatabaseNotInitialized,

    // --- Ingest ---
    #[error("parse error for {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("unsupported file format: {0}")]
    UnsupportedFormat(String),

    // --- Search ---
    #[error("search error: {0}")]
    Search(String),

    #[error("embedding error: {0}")]
    Embedding(#[source] anyhow::Error),

    // --- Vault ---
    #[error("vault I/O error: {0}")]
    VaultIo(#[from] std::io::Error),

    // --- Not Found ---
    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("turn not found: session={session_id} turn={turn_index}")]
    TurnNotFound {
        session_id: String,
        turn_index: u32,
    },

    // --- Config ---
    #[error("config error: {0}")]
    Config(String),

    // --- General (anyhow fallback) ---
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, SecallError>;
```

> `Other(anyhow::Error)` variant로 점진적 마이그레이션 가능. 모든 함수를 한 번에 바꾸지 않아도 컴파일 가능.

### Step 2: lib.rs에 모듈 추가

```rust
// crates/secall-core/src/lib.rs
pub mod error;
pub use error::{SecallError, Result};
```

### Step 3: 핵심 모듈 점진적 전환

**우선 전환 대상** (MCP 에러 분기에 직접 영향):

| 모듈 | 메서드 | 현재 → 변경 | variant |
|---|---|---|---|
| `db.rs:16` | `open()` | `anyhow::Result` → `Result` | `Database` / `DatabaseNotInitialized` |
| `db.rs:147` | `get_turn()` | `anyhow::Result` → `Result` | `TurnNotFound` |
| `bm25.rs:277` | `search_fts()` | `anyhow::Result` → `Result` | `Search` |
| `bm25.rs:328` | `get_session_meta()` | `anyhow::Result` → `Result` | `SessionNotFound` |
| `vector.rs:310` | `search_vectors()` | `anyhow::Result` → `Result` | `Database` |
| `hybrid.rs:76` | `search()` | `anyhow::Result` → `Result` | `Search` / `Embedding` |

**나머지 함수**: `Other(anyhow::Error)` 자동 변환으로 즉시 호환. 이후 task에서 점진적 전환.

### Step 4: MCP 서버 에러 매핑 개선 (server.rs)

```rust
// server.rs — 헬퍼 함수
fn to_mcp_error(e: SecallError) -> McpError {
    match &e {
        SecallError::SessionNotFound(_) | SecallError::TurnNotFound { .. } => {
            McpError::invalid_params(e.to_string(), None)
        }
        SecallError::DatabaseNotInitialized => {
            McpError::internal_error(e.to_string(), None)
        }
        SecallError::Parse { .. } | SecallError::UnsupportedFormat(_) => {
            McpError::invalid_params(e.to_string(), None)
        }
        _ => McpError::internal_error(e.to_string(), None),
    }
}
```

> 각 tool 메서드에서 `.map_err(to_mcp_error)?` 패턴으로 통일.

### Step 5: CLI 에러 처리 (bin crate)

bin crate(`crates/secall`)는 `main()`에서 `anyhow::Result`를 반환하므로, `SecallError`가 `Into<anyhow::Error>`를 구현하면 기존 CLI 동작에 영향 없음. thiserror의 `#[error(...)]`이 `Display`를 구현하므로 에러 메시지도 자동 개선.

## Dependencies

- `thiserror = "2"` — 이미 `Cargo.toml:18`에 workspace dependency 존재
- P3 CI 안전망 권장 (대규모 시그니처 변경이므로 회귀 감지 필요)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. SecallError 타입이 공개 API에 노출되는지 확인
cargo doc -p secall-core --no-deps 2>&1 | grep -c "SecallError"

# 4. MCP 서버 테스트 통과
cargo test -p secall-core mcp

# 5. clippy 통과
cargo clippy --all-targets -- -D warnings
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요.

## Risks

- **대규모 시그니처 변경**: 40+ public 함수의 반환 타입이 변경될 수 있다. `Other(anyhow::Error)` variant와 `From<anyhow::Error>` impl으로 점진적 전환이 가능하므로, 1차에서는 핵심 6개 메서드만 전환하고 나머지는 자동 변환에 의존.
- **anyhow `?` 호환성**: `SecallError`가 `From<anyhow::Error>`를 구현하면, 기존 코드의 `?` 연산자가 그대로 동작. 단, `anyhow::Context` trait의 `.context()`는 `SecallError` 반환 함수에서 직접 사용 불가 → `.map_err(SecallError::Other)?` 또는 `anyhow!()` 래핑 필요.
- **thiserror v2 호환성**: thiserror 2.x는 1.x와 API 호환이므로 문제 없음.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/query_expand.rs` — 쿼리 확장은 P3 Task 04 영역
- `crates/secall-core/src/vault/mod.rs` — Vault I/O 에러는 이 task에서 variant만 정의. 실제 전환은 후속.
- `crates/secall-core/src/hooks/mod.rs` — 훅 에러는 비치명적이므로 anyhow 유지
- `crates/secall-core/src/search/embedding.rs` — Embedder trait의 반환 타입은 async-trait 제약으로 anyhow 유지. `Embedding` variant는 hybrid.rs에서 래핑.
