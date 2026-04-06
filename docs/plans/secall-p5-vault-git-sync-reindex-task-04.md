---
type: task
status: draft
plan: secall-p5-vault-git-sync-reindex
task_number: 4
title: "host 필드 추가"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 04: host 필드 추가

## 문제

멀티기기 환경에서 세션이 어떤 기기에서 생성되었는지 추적할 수 없다. frontmatter에 호스트 정보가 없어 기기별 필터링이 불가능하다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/types.rs` | 수정 | `Session` struct (lines 24-35)에 `host: Option<String>` 추가 |
| `crates/secall-core/src/ingest/markdown.rs` | 수정 | `render_session()` (line 8)에서 frontmatter에 `host` 필드 추가 |
| `crates/secall-core/src/ingest/claude.rs` | 수정 | Session 생성부 (lines 275-286)에 `host` 할당 |
| `crates/secall-core/src/ingest/codex.rs` | 수정 | Session 생성부 (lines 231-242)에 `host` 할당 |
| `crates/secall-core/src/ingest/gemini.rs` | 수정 | Session 생성부 (lines 254-265)에 `host` 할당 |
| `crates/secall-core/src/store/schema.rs` | 수정 | sessions 테이블 (lines 3-22)에 `host TEXT` 컬럼 추가 |
| `crates/secall-core/src/store/db.rs` | 수정 | `migrate()` (lines 36-66)에 ALTER TABLE 추가, `insert_session()` (lines 191-230)에 host 포함 |
| `crates/secall-core/src/search/bm25.rs` | 수정 | `insert_session()` (SessionRepo impl)에 host 필드 포함 |
| `Cargo.toml` (workspace) | 수정 | `gethostname = "0.4"` workspace 의존성 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `gethostname.workspace = true` 추가 |

## Change description

### Step 1: gethostname 의존성 추가

```toml
# Cargo.toml (workspace root)
[workspace.dependencies]
gethostname = "0.4"

# crates/secall-core/Cargo.toml
[dependencies]
gethostname.workspace = true
```

### Step 2: Session 구조체에 host 추가

`crates/secall-core/src/ingest/types.rs` — Session struct (lines 24-35):

```rust
pub struct Session {
    pub id: String,
    pub agent: AgentKind,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub host: Option<String>,  // 추가
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub turns: Vec<Turn>,
    pub total_tokens: TokenCount,
}
```

### Step 3: 각 파서에서 hostname 자동 기록

`crates/secall-core/src/ingest/claude.rs` — Session 생성 (lines 275-286):
`crates/secall-core/src/ingest/codex.rs` — Session 생성 (lines 231-242):
`crates/secall-core/src/ingest/gemini.rs` — Session 생성 (lines 254-265):

```rust
use gethostname::gethostname;

Session {
    // ... 기존 필드들 ...
    host: Some(gethostname().to_string_lossy().to_string()),
}
```

> 세션 원본(JSONL/JSON)에는 hostname이 없으므로, **ingest 시점의 기기 이름**을 기록.

### Step 4: 마크다운 frontmatter에 host 추가

`crates/secall-core/src/ingest/markdown.rs` — `render_session()` (line 8):

```rust
// frontmatter 렌더링 부분에 추가
if let Some(host) = &session.host {
    fm.push_str(&format!("host: {host}\n"));
}
```

결과 예시:
```yaml
---
type: session
agent: claude-code
session_id: abc123
date: 2026-04-06
host: mac-office
---
```

### Step 5: DB 스키마에 host 컬럼 추가

`crates/secall-core/src/store/schema.rs` — CREATE_SESSIONS (lines 3-22):

```sql
-- sessions 테이블에 host 컬럼 추가
host TEXT,
```

`crates/secall-core/src/store/db.rs` — `migrate()` (lines 36-66)에 마이그레이션 추가:

```rust
// 기존 테이블에 host 컬럼이 없으면 추가
if !self.column_exists("sessions", "host")? {
    self.conn().execute("ALTER TABLE sessions ADD COLUMN host TEXT", [])?;
}
```

> `ALTER TABLE ADD COLUMN`은 SQLite에서 안전. 기존 행의 host는 NULL.

### Step 6: insert_session()에 host 포함

`crates/secall-core/src/store/db.rs` — `insert_session()` (lines 191-230, SessionRepo impl):

```rust
// INSERT 문에 host 컬럼 추가
rusqlite::params![
    session.id, session.agent.as_str(), session.model, session.project,
    session.cwd, session.git_branch, session.host,  // host 추가
    // ...
]
```

`crates/secall-core/src/search/bm25.rs` — SessionRepo의 `insert_session()`:

```rust
// bm25.rs의 index_session() 내 db.insert_session() 호출부 — 
// Session 구조체에 host가 포함되므로 자동 전달
```

## Dependencies

- `gethostname = "0.4"` — 신규 의존성
- 스키마 마이그레이션 포함 (ALTER TABLE)
- Task 01, 02, 03과 독립적으로 구현 가능

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. ingest 후 host 필드가 있는 MD 파일 확인
cargo run -p secall -- ingest --auto 2>&1 | head -5
# 이후 최신 생성 MD 파일에서 host 필드 확인:
# Manual: vault/raw/sessions/ 최신 파일 열어서 frontmatter에 host: 존재 확인

# 4. DB에 host 컬럼 존재 확인
sqlite3 ~/.config/secall/secall.db "PRAGMA table_info(sessions)" 2>/dev/null | grep host && echo "OK: host column exists"

# 5. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **기존 세션 호환**: 이미 ingest된 세션의 host는 NULL. 문제 없음 — 필터링 시 NULL은 제외.
- **hostname 변경**: 사용자가 기기 이름을 변경하면 같은 기기의 세션이 다른 host로 기록됨. 실용적으로 드문 케이스.
- **hostname 형식**: macOS는 `MacBook-Pro.local` 같은 형식. 표시 시 `.local` 제거 고려 (1차에서는 미구현).
- **프라이버시**: hostname이 git 원격에 공유됨. 민감한 경우 config에서 `host_alias` 오버라이드 고려 (후속 작업).
- **Session 구조체 변경 영향**: Session에 필드 추가 시 모든 파서에서 해당 필드를 설정해야 컴파일됨. claude/codex/gemini 3개 파서 모두 수정 필수.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/vault/git.rs` — Task 02 영역
- `crates/secall/src/commands/sync.rs` — Task 03 영역
- `crates/secall/src/commands/reindex.rs` — Task 01 영역
- `crates/secall-core/src/mcp/server.rs` — MCP 필터에 host 추가는 후속 작업
- `crates/secall-core/src/ingest/detect.rs` — 탐지 로직 변경 없음
- `crates/secall-core/src/ingest/lint.rs` — 린트 로직 변경 없음
