---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p35-secall-web-phase-3
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — 백엔드 `/api/tags` 엔드포인트

## Changed files

수정:
- `crates/secall-core/src/store/session_repo.rs:236` — `list_agents` 다음 줄에 `list_all_tags() -> Result<Vec<TagCount>>` 추가, 같은 파일 상단 적당한 위치에 `#[derive(Debug, Clone, Serialize)] pub struct TagCount { pub name: String, pub count: i64 }` 정의 (혹은 `mcp/dto.rs`가 있으면 거기에)
- `crates/secall-core/src/mcp/server.rs:580` — `do_list_agents` 다음 줄에 `do_list_tags(&self, with_counts: bool) -> Result<serde_json::Value>` 추가
- `crates/secall-core/src/mcp/rest.rs:144` — `/api/agents` 라우트 다음 줄에 `.route("/api/tags", get(api_list_tags))` 추가, 핸들러는 `api_list_agents`(rest.rs:366) 패턴을 그대로 따라 `query: Query<TagsListQuery>` 받아 `do_list_tags(query.with_counts.unwrap_or(true))` 호출

신규: 없음 (기존 파일들에 추가만)

## Change description

### 1. `list_all_tags` (DB 레이어)

`session_repo.rs`의 `list_agents` 바로 아래에 추가:

```rust
pub fn list_all_tags(&self) -> Result<Vec<TagCount>> {
    // sessions.tags는 JSON 배열 ('["rust","search"]' 형태). json_each로 펼침.
    // tags가 NULL이거나 빈 배열이면 결과에 안 잡힘 → 안전.
    let mut stmt = self.conn().prepare(
        "SELECT json_each.value AS tag, COUNT(*) AS cnt
         FROM sessions, json_each(sessions.tags)
         WHERE sessions.tags IS NOT NULL AND json_valid(sessions.tags)
         GROUP BY tag
         ORDER BY cnt DESC, tag ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(TagCount {
            name: r.get(0)?,
            count: r.get(1)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}
```

`TagCount` 구조체는 같은 파일 상단(예: 기존 `SessionListFilter` 구조체 근처)에 `#[derive(Debug, Clone, Serialize)] pub struct TagCount { pub name: String, pub count: i64 }`로 정의. `serde::Serialize` import는 이미 있음 (확인).

### 2. `do_list_tags` (MCP 서버)

`server.rs`의 `do_list_agents` 바로 아래에 추가:

```rust
pub fn do_list_tags(&self, with_counts: bool) -> anyhow::Result<serde_json::Value> {
    let db = self
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
    let tags = db.list_all_tags()?;
    if with_counts {
        Ok(serde_json::json!({ "tags": tags }))
    } else {
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        Ok(serde_json::json!({ "tags": names }))
    }
}
```

- `with_counts=true` (기본값): `{ "tags": [{ "name": "rust", "count": 12 }, ...] }`
- `with_counts=false`: `{ "tags": ["rust", "search", ...] }` — string 배열만

### 3. REST 라우트 + 핸들러

`rest.rs:143-144` 근처에 다음 라우트 추가:

```rust
.route("/api/tags", get(api_list_tags))
```

`SessionListQuery` 정의 근처에 `TagsListQuery` 추가 + 핸들러:

```rust
#[derive(Debug, Deserialize, Default)]
struct TagsListQuery {
    with_counts: Option<bool>,
}

async fn api_list_tags(
    State(s): State<Arc<SeCallMcpServer>>,
    Query(q): Query<TagsListQuery>,
) -> impl IntoResponse {
    match s.do_list_tags(q.with_counts.unwrap_or(true)) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}
```

`Query` import는 axum::extract::Query (rest.rs:1-15에 이미 있음).

### 4. 통합 테스트 추가

`crates/secall-core/tests/rest_listing.rs` 끝에 다음 테스트 추가:

```rust
#[test]
fn rest_list_all_tags_with_counts_desc_then_alpha() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s1", "p", 0)).unwrap();
    db.insert_session(&make_session("s2", "p", 1)).unwrap();
    db.insert_session(&make_session("s3", "p", 2)).unwrap();

    db.update_session_tags("s1", &["rust".into(), "alpha".into()])
        .unwrap();
    db.update_session_tags("s2", &["rust".into(), "search".into()])
        .unwrap();
    db.update_session_tags("s3", &["rust".into()]).unwrap();

    let tags = db.list_all_tags().unwrap();
    // rust(3) > alpha(1)/search(1) (alpha < search 알파벳)
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0].name, "rust");
    assert_eq!(tags[0].count, 3);
    assert_eq!(tags[1].name, "alpha");
    assert_eq!(tags[2].name, "search");
}

#[test]
fn rest_list_all_tags_excludes_null_and_empty_arrays() {
    let db = Database::open_memory().unwrap();
    db.insert_session(&make_session("s-null", "p", 0)).unwrap();
    db.insert_session(&make_session("s-empty", "p", 1)).unwrap();
    db.update_session_tags("s-empty", &[]).unwrap(); // 빈 배열 저장 가정
    let tags = db.list_all_tags().unwrap();
    assert!(tags.is_empty());
}
```

## Dependencies

- 외부 crate: 없음 (rusqlite + JSON1 extension은 bundled feature에 포함)
- 내부 task: 없음

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --test rest_listing rest_list_all_tags

# 라이브 (선택, 서버 실행 필요):
# secall serve --bind 127.0.0.1:8080 &
# curl -s http://127.0.0.1:8080/api/tags | jq '.tags[0]'
# curl -s 'http://127.0.0.1:8080/api/tags?with_counts=false' | jq '.tags | length'
```

## Risks

- **JSON1 SQLite extension**: `json_each`는 SQLite JSON1 확장 필요. rusqlite의 `bundled` feature가 JSON1 포함하므로 OK (workspace Cargo.toml `rusqlite = { version = "0.31", features = ["bundled"] }`로 확인됨).
- **빈 태그 배열 저장**: `update_session_tags`가 `[]`를 어떻게 저장하는지 확인 필요. NULL이면 json_each 통과, `'[]'` 문자열이면 json_each가 0행 반환 → 둘 다 안전.
- **태그 정규화**: P32 task에서 `normalize_tag` (소문자 + `-` 변환)가 저장 시점에 적용됨 → list_all_tags는 정규화된 형태만 반환 → 클라 측 추가 정규화 불필요.
- **count = i64 vs usize**: SQLite COUNT는 INTEGER → i64. JSON 직렬화에서 number로 감.
- **성능**: sessions 1만 건 + 평균 3 태그 = 3만 행 grouping. 인덱스 없어도 100ms 이내. 더 커지면 materialized view 또는 별도 tags 테이블 (Phase 4+).

## Scope boundary

수정 금지:
- `web/` 전체 — Task 01 영역
- `crates/secall-core/src/store/{db,schema,jobs_repo,tag_normalize}.rs` — 본 task와 무관
- `crates/secall-core/src/jobs/`, `crates/secall-core/src/web/` — 무관
- `crates/secall/`, `.github/`, `README*` — 무관
