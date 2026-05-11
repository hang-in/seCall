---
type: task
plan_slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
task_id: 05
title: 기본 list / search / recall / hybrid 에 is_archived = 0 filter 적용
parallel_group: B
depends_on: [01]
status: pending
updated_at: 2026-05-12
---

# Task 05 — 기본 list / search / recall filter

## Changed files

수정:

- `crates/secall-core/src/store/session_repo.rs:796-` 의 `list_sessions_filtered` — `conditions` 기본값에 `"is_archived = 0".to_string()` 추가. `SessionListFilter` 에 `pub include_archived: bool` (기본 false) 추가하고 true 시 해당 조건 생략. SessionListItem 에 `pub is_archived: bool`, `pub archived_at: Option<String>` 직렬화 필드 추가 + SELECT 컬럼 / row 매핑 갱신.
- `crates/secall-core/src/store/session_repo.rs:1101-` 의 `SessionListFilter` struct — `pub include_archived: bool` 추가.
- `crates/secall-core/src/store/session_repo.rs:1116-` 의 `SessionListItem` struct — `pub is_archived: bool`, `pub archived_at: Option<String>` 추가.
- `crates/secall-core/src/search/bm25.rs:116-` 의 `search` — turns_fts MATCH 결과를 sessions 와 JOIN 해 `WHERE sessions.is_archived = 0` 추가. 옵션 인자 `include_archived: bool` 추가 또는 별도 옵션 struct.
- `crates/secall-core/src/search/hybrid.rs:126-` 의 `search` / `search_bm25` / `search_vector` — 위와 동일하게 archive filter 옵션 전파.
- `crates/secall-core/src/search/vector.rs:240-` 의 `search` — 벡터 결과 후 sessions JOIN 으로 archived 제외.
- `crates/secall-core/src/mcp/server.rs:1056-` 의 `recall` — 기본 archived 제외 (`include_archived: false`) 적용. 기존 클라이언트에 새 옵션 노출은 P46 영역.

신규:

- 없음.

회귀 테스트:

- `crates/secall-core/src/store/session_repo.rs` 의 회귀 (또는 `tests/session_list_archived.rs` 신규):
  1. `test_list_sessions_filtered_excludes_archived_by_default` — archived 세션 1개 + 일반 1개 → default list 에 일반만.
  2. `test_list_sessions_filtered_include_archived_true_returns_all` — include_archived=true → 둘 다.
- BM25 회귀:
  3. `test_bm25_search_excludes_archived` — archived 세션의 본문이 검색 hits 에서 제외.
- Hybrid 회귀:
  4. `test_hybrid_search_excludes_archived` — RRF 결과에서도 archived 제외.

## Change description

### 1. SessionListFilter / Item 확장

```rust
#[derive(Debug, Default, Clone)]
pub struct SessionListFilter {
    // ... 기존 필드 ...
    pub favorite: Option<bool>,
    pub q: Option<String>,
    pub page: usize,
    pub page_size: usize,
    /// P45 — true 면 archived 세션 포함. 기본 false (제외).
    pub include_archived: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionListItem {
    // ... 기존 필드 ...
    pub notes: Option<String>,
    /// P45
    pub is_archived: bool,
    pub archived_at: Option<String>,
}
```

`list_sessions_filtered` 내부:

```rust
let mut conditions: Vec<String> = vec![
    "session_type != 'automated'".to_string(),
];
if !f.include_archived {
    conditions.push("is_archived = 0".to_string());
}
// ... 기존 다른 조건들 ...
```

SELECT / row 매핑에 `is_archived, archived_at` 추가.

### 2. BM25 search — JOIN

`bm25.rs:116` 의 search 가 사용하는 SQL (보통 `search_repo::search_fts`) 에 sessions JOIN 추가:

```sql
SELECT t.session_id, t.turn_id, t.content, ...
FROM turns_fts t
JOIN sessions s ON s.id = t.session_id
WHERE t MATCH ?1
  AND (?2 = 1 OR s.is_archived = 0)   -- ?2 = include_archived 0/1
ORDER BY rank LIMIT ?3
```

> `turns_fts` 의 `session_id UNINDEXED` 필드는 이미 schema.rs:50 에 존재 — JOIN 가능.
> rusqlite `prepare_cached` 사용 시 같은 SQL 캐시 활용 — `include_archived` 를 항상 bind 변수로 전달해 SQL 한 가지로 통일.

함수 시그니처 변형 옵션:

```rust
pub fn search(
    &self,
    query: &str,
    limit: usize,
    include_archived: bool,
) -> Result<Vec<Hit>> { ... }
```

caller (recall, REST search route) 가 명시 — 기본 호출은 `include_archived = false`.

### 3. Hybrid / Vector search

같은 패턴. `search_repo::search_vectors` 와 `search_bm25` 양쪽에 동일 옵션 전파. RRF 결합 전에 각 source 가 archived 제외하므로 RRF 결과도 자연스럽게 archived 0건.

벡터 결과는 `turn_vectors` 테이블 기반. `turn_vectors` 는 session 관계 없이 turn_id 만 가질 수 있음 → vector hits 의 turn_id 로 sessions JOIN 후 archive 제외.

### 4. recall (MCP)

`mcp/server.rs:1056` 의 `recall` 메서드 — 내부적으로 hybrid search 호출. caller 옵션 `include_archived: bool` 을 MCP request param 으로 받지만 기본 false. P46 (REST recall) 영역과 일치.

본 task 에선 MCP 의 `recall` 진입에서 `include_archived = false` 명시 + 함수 시그니처 확장만. MCP request param 의 명시적 노출은 P46.

### 5. graph snapshot — 영역 외

`list_sessions_for_graph_rebuild` (session_repo.rs:999) 와 graph snapshot 의 archive 필터는 **P49** 에서 처리. 본 task 는 list / search / recall / hybrid 만.

### 6. partial index 활용

task 01 의 `idx_sessions_archived ... WHERE is_archived = 1` 은 archive 된 row 만 매핑. 기본 검색 (`WHERE is_archived = 0`) 은 sessions 의 full-row scan 이지만 sessions 자체가 작아 비용 미미. SQLite query planner 가 `is_archived = 0` 인 row 가 대다수일 때 index 우회 후 sequential scan 선택 → 최적.

## Dependencies

- task 01 — `is_archived` 컬럼이 schema 에 존재.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. list_sessions_filtered 회귀
cargo test -p secall-core --lib store::session_repo::tests::test_list_sessions_filtered_excludes_archived
cargo test -p secall-core --lib store::session_repo::tests::test_list_sessions_filtered_include_archived

# 3. BM25 / hybrid 회귀
cargo test -p secall-core --lib search::bm25::tests::test_bm25_search_excludes_archived
cargo test -p secall-core --lib search::hybrid::tests::test_hybrid_search_excludes_archived

# 4. 기존 search / list 회귀 (archived 없는 데이터에선 동작 동일)
cargo test -p secall-core --lib search::
cargo test -p secall-core --lib store::session_repo
```

## Risks

- **SessionListItem 의 새 필드 → JSON 응답 변경** — REST / Web 클라이언트가 strict deserialize 면 break. mitigation: 모두 추가 필드이므로 default skip 가능 (`#[serde(default)]`). 본 task 는 추가만 — 기존 클라이언트 무영향.
- **함수 시그니처 변경 → caller 전체 갱신** — `bm25::search`, `hybrid::search` 등 caller 가 많음. mitigation: option struct 패턴 (`SearchOptions { include_archived: bool }`) 으로 모아 점진 확장 가능하게. 본 task 는 단순 bool 인자 추가 — caller 5-10곳 갱신.
- **FTS5 + JOIN 성능 회귀** — sessions 의 row 수가 turns 보다 훨씬 적어 (1:N) JOIN 비용 미미. seCall 의 sessions 는 보통 수천 건. SQLite 의 hash join 으로 처리. 별도 성능 측정 없이 진행 OK.
- **`is_archived = 0` 의 partial index 미사용** — 의도된 동작. 0쪽 partial index 만들면 row 대다수 포함해 비효율.
- **vault re-ingest 시 archive 상태 sync 지연** — sync → ingest 사이 잠시 동안 DB 와 vault 의 archive 상태 불일치 가능. mitigation: task 03 의 INSERT OR IGNORE + UPDATE archive 로 ingest 단계에서 강제 sync. 사용자 입장에서 sync 호출 직후 정합성 회복.

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/schema.rs` — task 01 영역.
- `crates/secall-core/src/store/session_repo.rs::archive_session` / `restore_session` — task 04 영역.
- `crates/secall-core/src/ingest/*` — task 02 / 03 영역.
- `crates/secall-core/src/store/session_repo.rs::list_sessions_for_graph_rebuild` — graph 영역 (P49).
- `crates/secall-core/src/wiki/*` — wiki 영역 (P49).
- `crates/secall-core/src/graph/*` — graph 영역 (P49).
- `crates/secall/src/commands/*` — REST / CLI (P46).
- `web/*` — Web UI (P46).
