---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — DB 스키마 v7 (`notes` 컬럼) + REST PATCH

## Changed files

수정:
- `crates/secall-core/src/store/schema.rs:1` — `CURRENT_SCHEMA_VERSION = 7`
- `crates/secall-core/src/store/schema.rs:CREATE_SESSIONS` — `notes TEXT` 컬럼 추가
- `crates/secall-core/src/store/db.rs` (마이그레이션) — `if current < 7` 분기 + `ALTER TABLE sessions ADD COLUMN notes TEXT`
- `crates/secall-core/src/store/session_repo.rs` — `update_session_notes(session_id, notes) -> Result<()>` 신규, `SessionListItem`/`get_session_list_item` 응답에 `notes: Option<String>` 추가, `do_get` 응답 보강에 notes 포함
- `crates/secall-core/src/mcp/server.rs` — `do_set_notes(session_id, notes)` 신규 + `do_get`이 `notes`를 응답에 추가
- `crates/secall-core/src/mcp/rest.rs` — `PATCH /api/sessions/{id}/notes` 라우트 + 핸들러
- `crates/secall-core/src/store/db.rs` (테스트 모듈) — v7 마이그레이션 테스트, `update_session_notes` 단위 테스트

신규: 없음

## Change description

### 1. 스키마 v7

```rust
pub const CURRENT_SCHEMA_VERSION: u32 = 7;

pub const CREATE_SESSIONS: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    ...
    is_favorite   INTEGER DEFAULT 0,
    notes         TEXT
);
";
```

### 2. 마이그레이션 분기

`db.rs`의 `migrate()`에서 v6 분기 다음에:
```rust
if current < 7 && !self.column_exists("sessions", "notes")? {
    self.conn.execute(
        "ALTER TABLE sessions ADD COLUMN notes TEXT",
        [],
    )?;
}
```

### 3. `update_session_notes`

```rust
impl Database {
    pub fn update_session_notes(
        &self,
        session_id: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let affected = self.conn().execute(
            "UPDATE sessions SET notes = ?1 WHERE id = ?2",
            rusqlite::params![notes, session_id],
        )?;
        if affected == 0 {
            return Err(SecallError::SessionNotFound(session_id.to_string()));
        }
        Ok(())
    }
}
```

빈 문자열은 그대로 저장 (NULL 변환 안 함). 사용자가 명시적으로 비우면 ""로 저장.

### 4. `SessionListItem` + `get_session_list_item` + `do_get`

`SessionListItem` 구조체에 `pub notes: Option<String>` 추가. SELECT 컬럼 목록에 `notes` 추가. JSON 직렬화에 자동 포함.

`do_get`에서 `notes`도 응답 보강:
```rust
if let Some(n) = item.notes {
    json_val["notes"] = serde_json::Value::String(n);
}
```

### 5. REST 라우트

```rust
.route("/api/sessions/{id}/notes", patch(api_set_notes))
```

```rust
#[derive(Deserialize)]
struct SetNotesBody { notes: Option<String> }

async fn api_set_notes(
    State(s): State<Arc<SeCallMcpServer>>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetNotesBody>,
) -> impl IntoResponse {
    match s.do_set_notes(&id, body.notes.as_deref()) {
        Ok(_) => (StatusCode::OK, Json(json!({ "session_id": id, "notes": body.notes }))).into_response(),
        Err(e) => error_response(e),
    }
}
```

### 6. 단위 테스트

`db.rs` tests에 추가:
- `test_v7_notes_column_exists` — 신규 DB jobs 테이블 + notes 컬럼 모두 OK
- `test_v7_migrates_v6_db` — v6 DB에서 ALTER TABLE 적용
- `test_update_session_notes_sets_and_clears` — set/clear/null 패턴 검증
- `test_update_session_notes_missing_session` — Err

`tests/rest_listing.rs` 또는 신규 `tests/notes_rest.rs`에 PATCH /api/sessions/:id/notes 통합 테스트.

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음 (root)

## Verification

```bash
cargo check -p secall-core --all-features
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib store::db::tests::test_v7
cargo test -p secall-core --lib store::db::tests::test_update_session_notes
cargo test --all
```

## Risks

- **NOTE 길이 제한 없음**: 매우 큰 notes (수MB)도 INSERT 가능. SQLite TEXT 한계는 1GB지만 응답 페이로드 비대화 위험. P35에서 길이 제한 검토
- **Obsidian 플러그인 호환**: `do_get` 응답에 `notes` 추가 — 기존 클라이언트는 무시 (옵셔널 필드)
- **빈 문자열 vs NULL**: 빈 문자열 저장은 의도된 동작. 클라이언트가 NULL과 ""를 구분해야 함
- **마이그레이션 ALTER ADD COLUMN**: SQLite 3.35+ 안전. 기존 row는 NULL로 채워짐

## Scope boundary

수정 금지:
- `crates/secall-core/src/jobs/`, `web/`, `.github/`, `README*`
- 기존 v1~v6 마이그레이션 분기 (변경 금지, v7만 추가)
- 기존 `update_session_*` 메서드 시그니처 (notes만 신규)
