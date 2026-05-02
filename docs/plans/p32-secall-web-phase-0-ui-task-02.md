---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 02
parallel_group: C
depends_on: [03]
---

# Task 02 — 신규 REST 엔드포인트 추가

## Changed files

수정:
- `crates/secall-core/src/mcp/rest.rs:101-110` — 5개 신규 라우트 등록
- `crates/secall-core/src/mcp/rest.rs` (끝부분) — 신규 핸들러 함수 추가
- `crates/secall-core/src/mcp/server.rs` — `do_list_sessions()`, `do_list_projects()`, `do_list_agents()`, `do_set_tags()`, `do_set_favorite()` 메서드 추가
- `crates/secall-core/src/store/session_repo.rs` — `list_sessions_filtered()`, `update_session_tags()`, `update_session_favorite()` 메서드 추가 (`is_favorite` 컬럼은 Task 04에서 추가됨)

신규:
- `crates/secall-core/tests/rest_listing.rs` — 신규 엔드포인트 통합 테스트

## Change description

### 1. 엔드포인트 사양

| Method | Path | Body / Query | Response |
|---|---|---|---|
| `GET` | `/api/sessions` | query: `page`, `page_size`, `project`, `agent`, `date_from`, `date_to`, `tag`, `favorite`, `q` | `{ items: [...], total, page, page_size }` |
| `GET` | `/api/projects` | — | `{ projects: [string, ...] }` |
| `GET` | `/api/agents` | — | `{ agents: [string, ...] }` |
| `PATCH` | `/api/sessions/:id/tags` | `{ tags: [string, ...] }` | `{ session_id, tags: [normalized] }` |
| `PATCH` | `/api/sessions/:id/favorite` | `{ favorite: bool }` | `{ session_id, favorite: bool }` |

### 2. `crates/secall-core/src/store/session_repo.rs` 신규 메서드

#### `list_sessions_filtered`
```rust
pub struct SessionListFilter {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub date_from: Option<String>,   // "YYYY-MM-DD"
    pub date_to: Option<String>,
    pub tag: Option<String>,         // 단일 태그 매칭 (JSON LIKE)
    pub favorite: Option<bool>,
    pub q: Option<String>,           // summary LIKE
    pub page: usize,                 // 1-based
    pub page_size: usize,            // 기본 30, 최대 100
}

pub struct SessionListItem {
    pub id: String,
    pub agent: String,
    pub project: Option<String>,
    pub model: Option<String>,
    pub date: String,                // start_time의 YYYY-MM-DD
    pub start_time: String,
    pub turn_count: i64,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub session_type: String,
    pub vault_path: Option<String>,
}

pub struct SessionListPage {
    pub items: Vec<SessionListItem>,
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
}

impl Database {
    pub fn list_sessions_filtered(&self, f: &SessionListFilter) -> Result<SessionListPage> {
        // dynamic WHERE 조립 (rusqlite 파라미터 바인딩)
        // ORDER BY start_time DESC
        // 태그 필터: JSON LIKE 매칭 (tags TEXT는 JSON 배열)
        //   → "tags LIKE '%\"<tag>\"%'" 패턴 (간단). 정밀하면 json_each 사용 가능
        // is_favorite: 0/1 비교
        // total은 동일 WHERE 절로 COUNT(*) 1회 추가 쿼리
    }
}
```

> 정확한 SQL은 동적 조립 — 빈 필터는 WHERE 생략. `automated` session_type은 기본 제외 (Obsidian 플러그인의 `do_recall`과 일관성).

#### `update_session_tags`
```rust
impl Database {
    pub fn update_session_tags(&self, session_id: &str, tags: &[String]) -> Result<Vec<String>> {
        // 1. 정규화: lowercase + 공백→'-' + 길이 제한 32자 + 불법 문자 제거
        let normalized: Vec<String> = tags.iter().map(normalize_tag).filter(|t| !t.is_empty()).collect();
        let json = serde_json::to_string(&normalized)?;
        self.conn().execute(
            "UPDATE sessions SET tags = ?1 WHERE id = ?2",
            rusqlite::params![json, session_id],
        )?;
        Ok(normalized)
    }
}

fn normalize_tag(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    let replaced = lower.chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();
    replaced.chars().take(32).collect()
}
```

> Task 04에서 동일한 `normalize_tag` 유틸을 `insert_session` 경로에서도 사용 가능하게 분리할 수 있음 (Task 04와 조율).

#### `update_session_favorite`
```rust
impl Database {
    pub fn update_session_favorite(&self, session_id: &str, favorite: bool) -> Result<()> {
        self.conn().execute(
            "UPDATE sessions SET is_favorite = ?1 WHERE id = ?2",
            rusqlite::params![favorite as i64, session_id],
        )?;
        Ok(())
    }
}
```

> `is_favorite` 컬럼은 Task 04에서 추가. Task 03은 컬럼 존재 가정.

### 3. `crates/secall-core/src/mcp/server.rs` 신규 메서드

```rust
impl SeCallMcpServer {
    pub fn do_list_sessions(&self, params: SessionListParams) -> anyhow::Result<serde_json::Value> {
        let filter = SessionListFilter::from(params);
        let db = self.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        let page = db.list_sessions_filtered(&filter)?;
        Ok(serde_json::to_value(page)?)
    }

    pub fn do_list_projects(&self) -> anyhow::Result<serde_json::Value> {
        let db = self.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        Ok(serde_json::json!({ "projects": db.list_projects()? }))
    }

    pub fn do_list_agents(&self) -> anyhow::Result<serde_json::Value> {
        let db = self.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        Ok(serde_json::json!({ "agents": db.list_agents()? }))
    }

    pub fn do_set_tags(&self, session_id: &str, tags: Vec<String>) -> anyhow::Result<serde_json::Value> {
        let db = self.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        let normalized = db.update_session_tags(session_id, &tags)?;
        Ok(serde_json::json!({ "session_id": session_id, "tags": normalized }))
    }

    pub fn do_set_favorite(&self, session_id: &str, favorite: bool) -> anyhow::Result<serde_json::Value> {
        let db = self.db.lock().map_err(|_| anyhow::anyhow!("db lock"))?;
        db.update_session_favorite(session_id, favorite)?;
        Ok(serde_json::json!({ "session_id": session_id, "favorite": favorite }))
    }
}
```

### 4. `crates/secall-core/src/mcp/rest.rs` 라우트/핸들러 추가

라우터에 추가 (`rest_router()`):
```rust
.route("/api/sessions", get(api_list_sessions))
.route("/api/projects", get(api_list_projects))
.route("/api/agents", get(api_list_agents))
.route("/api/sessions/:id/tags", patch(api_set_tags))
.route("/api/sessions/:id/favorite", patch(api_set_favorite))
```

핸들러:
```rust
#[derive(Debug, Deserialize)]
struct SessionListQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    project: Option<String>,
    agent: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    tag: Option<String>,
    favorite: Option<bool>,
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetTagsBody {
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SetFavoriteBody {
    favorite: bool,
}

async fn api_list_sessions(State(s): State<AppState>, Query(q): Query<SessionListQuery>) -> impl IntoResponse {
    match s.do_list_sessions(q.into()) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_list_projects(State(s): State<AppState>) -> impl IntoResponse {
    match s.do_list_projects() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_list_agents(State(s): State<AppState>) -> impl IntoResponse {
    match s.do_list_agents() {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_set_tags(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetTagsBody>,
) -> impl IntoResponse {
    match s.do_set_tags(&id, body.tags) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn api_set_favorite(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SetFavoriteBody>,
) -> impl IntoResponse {
    match s.do_set_favorite(&id, body.favorite) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => error_response(e),
    }
}
```

> `axum::extract::{Query, Path as AxumPath}`, `axum::routing::patch` import 필요.

### 5. 통합 테스트 `crates/secall-core/tests/rest_listing.rs` 신규

기존 `Database::open` 패턴으로 임시 DB 생성, 샘플 세션 1-2개 insert, 핸들러 메서드 직접 호출 (axum router 없이 `do_*()` 검증). 테스트 케이스:
- `do_list_sessions` 페이지네이션
- 프로젝트 필터
- 태그 필터
- favorite 필터
- `do_set_tags` 정규화 (대문자→소문자, 공백→`-`)
- `do_set_favorite` toggle

## Dependencies

- Task 04 (DB 스키마 v5) — `is_favorite` 컬럼이 존재해야 함
- 외부 crate 추가 없음 (`axum::extract::Query` 등 기존)

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core --all-features
cargo check -p secall

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 테스트
cargo test -p secall-core --test rest_listing

# 4. 기존 테스트 회귀 없음
cargo test --all

# 5. 라이브 검증 (release 빌드 후)
cargo build --release -p secall
./target/release/secall serve --port 18081 &
SERVER_PID=$!
sleep 2

# GET /api/projects
curl -s http://127.0.0.1:18081/api/projects | jq .

# GET /api/agents
curl -s http://127.0.0.1:18081/api/agents | jq .

# GET /api/sessions
curl -s "http://127.0.0.1:18081/api/sessions?page=1&page_size=5" | jq .

# PATCH 태그 (실제 세션 ID는 환경에 따라 다름 — 위에서 받은 ID 사용)
SID=$(curl -s "http://127.0.0.1:18081/api/sessions?page=1&page_size=1" | jq -r '.items[0].id')
if [ -n "$SID" ] && [ "$SID" != "null" ]; then
  curl -s -X PATCH "http://127.0.0.1:18081/api/sessions/$SID/tags" \
    -H "Content-Type: application/json" \
    -d '{"tags":["Rust","SEARCH","indexing"]}' | jq .
  # 응답에 정규화된 ["rust","search","indexing"] 기대

  curl -s -X PATCH "http://127.0.0.1:18081/api/sessions/$SID/favorite" \
    -H "Content-Type: application/json" \
    -d '{"favorite":true}' | jq .
fi

kill $SERVER_PID 2>/dev/null || true
```

## Risks

- **태그 LIKE 검색 정밀도**: `tags LIKE '%"<tag>"%'` 패턴은 부분 일치 위험 (예: `rust` 검색 시 `rust-lang` 매칭 가능). MVP에서는 허용, 정밀하면 `json_each(tags)` JOIN으로 교체
- **page_size 상한**: 100 초과 요청 시 silent clamp. 명시적 400 반환할지 결정 필요 — 본 task는 silent clamp
- **자동화 세션 노출**: `automated` session_type은 기본 제외. 명시적으로 보고 싶으면 `?session_type=automated` 추가 가능 (MVP 미포함)
- **인증 없음**: loopback이라 안전하지만, `PATCH` 엔드포인트는 데이터 변경 — 향후 multi-user 확장 시 권한 체크 필요
- **DB lock 경합**: 모든 핸들러가 `Mutex<Database>` 잠금. 동시 요청 시 직렬화. SQLite WAL 모드면 read는 비잠금이지만 현재 구조는 Mutex로 통제

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/schema.rs`, `db.rs` — Task 04 (스키마 변경)
- `crates/secall-core/src/web/`, `lib.rs` — Task 02
- `web/` — Task 05~08
- 기존 6개 핸들러 (`api_recall`, `api_get`, `api_status`, `api_wiki`, `api_graph`, `api_daily`) 시그니처 변경 — Obsidian 호환성 유지
- `.github/workflows/`, `README.md` — Task 09
