---
type: task
status: draft
plan: secall-refactor-p3
task_number: 4
title: "쿼리 확장 캐싱"
parallel_group: B
depends_on: [1]
updated_at: 2026-04-06
---

# Task 04: 쿼리 확장 캐싱

## 문제

`query_expand.rs:20-22`에서 `expand_query()`가 호출될 때마다 `claude -p` subprocess를 스폰한다. 동일 쿼리 반복 검색 시 불필요한 수 초 지연과 LLM API 비용이 발생한다.

### 현재 코드

```rust
// query_expand.rs:20-22
let output = std::process::Command::new("claude")
    .args(["-p", &prompt, "--model", "claude-haiku-4-5-20251001"])
    .output()?;
```

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/store/schema.rs` | 수정 | `query_cache` 테이블 DDL 추가 |
| `crates/secall-core/src/store/db.rs` | 수정 | 캐시 CRUD 메서드 추가 (2개) |
| `crates/secall-core/src/search/query_expand.rs` | 수정 | 캐시 lookup/store 로직 추가 |
| `crates/secall/src/commands/recall.rs` | 수정 | `expand_query()`에 `&db` 파라미터 전달 |

## Change description

### Step 1: query_cache 테이블 DDL (schema.rs)

```sql
-- schema.rs에 추가
CREATE TABLE IF NOT EXISTS query_cache (
    query_hash  TEXT PRIMARY KEY,
    original    TEXT NOT NULL,
    expanded    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

`migrate()` 내에서 기존 DDL 실행 후 추가 실행. 별도 스키마 버전 증가 불필요 (CREATE IF NOT EXISTS).

### Step 2: Database에 캐시 메서드 추가 (db.rs)

```rust
impl Database {
    /// 캐시에서 확장된 쿼리 조회. TTL 7일 초과 시 None.
    pub fn get_query_cache(&self, query: &str) -> Option<String> {
        let hash = Self::query_hash(query);
        self.conn()
            .query_row(
                "SELECT expanded FROM query_cache
                 WHERE query_hash = ?1
                   AND datetime(created_at, '+7 days') > datetime('now')",
                [&hash],
                |row| row.get(0),
            )
            .ok()
    }

    /// 확장 결과를 캐시에 저장.
    pub fn set_query_cache(&self, query: &str, expanded: &str) -> Result<()> {
        let hash = Self::query_hash(query);
        self.conn().execute(
            "INSERT OR REPLACE INTO query_cache(query_hash, original, expanded, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![hash, query, expanded],
        )?;
        Ok(())
    }

    fn query_hash(query: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        query.Hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
```

### Step 3: expand_query()에 캐시 연동 (query_expand.rs)

```rust
// query_expand.rs — 변경 후
pub fn expand_query(query: &str, db: Option<&Database>) -> Result<String> {
    // 1. 캐시 히트 확인
    if let Some(db) = db {
        if let Some(cached) = db.get_query_cache(query) {
            tracing::info!(query, "query expansion cache hit");
            return Ok(format!("{query} {cached}"));
        }
    }

    if !command_exists("claude") {
        tracing::warn!("claude not found, using original query");
        return Ok(query.to_string());
    }

    let prompt = format!(
        "다음 검색 쿼리를 확장해주세요. \
         원본 쿼리의 키워드, 동의어, 관련 기술 용어, 영어/한국어 변환을 포함하세요. \
         결과는 공백으로 구분된 키워드만 출력하세요. 설명 없이 키워드만.\n\n\
         쿼리: {query}"
    );

    let output = std::process::Command::new("claude")
        .args(["-p", &prompt, "--model", "claude-haiku-4-5-20251001"])
        .output()?;

    if output.status.success() {
        let expanded = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !expanded.is_empty() {
            // 2. 캐시 저장
            if let Some(db) = db {
                if let Err(e) = db.set_query_cache(query, &expanded) {
                    tracing::warn!(error = %e, "failed to cache query expansion");
                }
            }
            tracing::info!(original = query, expanded = %expanded, "query expanded");
            Ok(format!("{query} {expanded}"))
        } else {
            Ok(query.to_string())
        }
    } else {
        tracing::warn!("query expansion failed, using original query");
        Ok(query.to_string())
    }
}
```

> `db` 파라미터를 `Option`으로 받아 테스트 등에서 DB 없이도 호출 가능. 기존 API 하위 호환.

### Step 4: recall.rs에서 db 전달

```rust
// recall.rs — expand_query 호출부
// 변경 전
let expanded = expand_query(&query_str)?;
// 변경 후
let expanded = expand_query(&query_str, Some(&db))?;
```

### Step 5: 기존 테스트 수정

```rust
// query_expand.rs tests — 기존 테스트의 시그니처 변경
fn test_expand_query_no_claude() {
    // db 없이 호출
    let result = expand_query("벡터 검색", None).unwrap();
    assert_eq!(result, "벡터 검색");
}
```

새 테스트 추가:

```rust
#[test]
fn test_query_cache_hit() {
    let db = Database::open_memory().unwrap();
    // 테이블 생성 (migrate에서 자동)
    db.set_query_cache("벡터 검색", "vector search semantic embedding").unwrap();

    let cached = db.get_query_cache("벡터 검색");
    assert!(cached.is_some());
    assert!(cached.unwrap().contains("vector search"));
}

#[test]
fn test_query_cache_miss() {
    let db = Database::open_memory().unwrap();
    let cached = db.get_query_cache("존재하지 않는 쿼리");
    assert!(cached.is_none());
}
```

## Dependencies

- **Task 01 (CI/CD)**: 캐시 테이블 DDL 추가가 스키마 변경이므로 CI 안전망 확보 후 진행 권장
- `rusqlite`는 이미 의존성에 포함

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. query_expand 테스트 통과
cargo test -p secall-core query_expand

# 3. DB 관련 테스트 통과
cargo test -p secall-core db

# 4. 전체 테스트 회귀 없음
cargo test --all

# 5. 캐시 테이블 생성 확인 (수동)
# Manual: `sqlite3 <db_path> ".tables"` 실행 후 query_cache 테이블 존재 확인
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요. 형식: `✅ 명령 — exit 0` 또는 `❌ 명령 — 에러 내용 (사유)`. 검증 증빙 미제출 시 리뷰에서 conditional 처리됩니다.

## Risks

- **스키마 마이그레이션**: `CREATE TABLE IF NOT EXISTS`이므로 기존 DB에 영향 없음. 별도 버전 관리 불필요.
- **해시 충돌**: `DefaultHasher`의 64비트 해시는 충돌 확률이 극히 낮음. 캐시 용도이므로 충돌 시 최악의 경우 잘못된 확장 결과 반환 → 검색 품질에만 영향, 데이터 무결성에는 영향 없음.
- **TTL 정확도**: SQLite `datetime()` 비교는 초 단위. 실용적으로 7일 TTL에서 초 단위 오차는 무의미.
- **expand_query 시그니처 변경**: `Option<&Database>` 추가. 기존 호출자(recall.rs)만 수정하면 됨. MCP 서버에서도 expand가 사용될 경우 동일 패턴 적용.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/embedding.rs` — Task 02 영역
- `crates/secall-core/src/search/hybrid.rs` — Task 03 영역
- `crates/secall-core/src/mcp/server.rs` — MCP에서의 expand 연동은 이 task 범위 외 (현재 MCP는 expand 미사용)
