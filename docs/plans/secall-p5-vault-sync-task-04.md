---
type: task
status: draft
plan: secall-p5-vault-sync
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
| `crates/secall-core/src/ingest/types.rs` | 수정 | `Session` 구조체에 `host: Option<String>` 추가 |
| `crates/secall-core/src/ingest/markdown.rs` | 수정 | frontmatter 렌더링에 `host` 필드 추가 |
| `crates/secall-core/src/ingest/claude.rs` | 수정 | 파싱 시 `hostname` 자동 기록 |
| `crates/secall-core/src/ingest/codex.rs` | 수정 | 파싱 시 `hostname` 자동 기록 |
| `crates/secall-core/src/ingest/gemini.rs` | 수정 | 파싱 시 `hostname` 자동 기록 |
| `crates/secall-core/src/store/schema.rs` | 수정 | sessions 테이블에 `host` 컬럼 추가 |
| `crates/secall-core/src/search/bm25.rs` | 수정 | `insert_session()`에 host 필드 포함 |
| `Cargo.toml` (workspace) | 수정 | `gethostname` 의존성 추가 |

## Change description

### Step 1: Session 구조체에 host 추가

```rust
// ingest/types.rs — Session 구조체
pub struct Session {
    pub id: String,
    pub agent: AgentKind,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub host: Option<String>,  // 추가
    pub start_time: DateTime<Utc>,
    // ...
}
```

### Step 2: 파서에서 hostname 자동 기록

```rust
// claude.rs, codex.rs, gemini.rs — 각 파서의 Session 생성부
use gethostname::gethostname;

Session {
    // ... 기존 필드들 ...
    host: Some(gethostname().to_string_lossy().to_string()),
}
```

> 세션 원본(JSONL/JSON)에는 hostname이 없으므로, **ingest 시점의 기기 이름**을 기록. 이는 "이 세션을 이 기기에서 수집했다"는 의미.

### Step 3: 마크다운 frontmatter에 host 추가

```rust
// markdown.rs — render_frontmatter()
fn render_frontmatter(session: &Session) -> String {
    let mut fm = format!(
        "---\ntype: session\nagent: {}\n...",
        // ... 기존 필드들 ...
    );
    if let Some(host) = &session.host {
        fm.push_str(&format!("host: {host}\n"));
    }
    fm.push_str("---\n");
    fm
}
```

결과:
```yaml
---
type: session
agent: claude-code
session_id: abc123
date: 2026-04-06
host: mac-office     # ← 새 필드
---
```

### Step 4: DB 스키마에 host 컬럼 추가

```sql
-- schema.rs — sessions 테이블
ALTER TABLE sessions ADD COLUMN host TEXT;
```

마이그레이션 로직:
```rust
// db.rs — migrate() 내부
// 기존 테이블에 host 컬럼이 없으면 추가
if !self.column_exists("sessions", "host")? {
    self.conn().execute("ALTER TABLE sessions ADD COLUMN host TEXT", [])?;
}
```

> `ALTER TABLE ADD COLUMN`은 SQLite에서 안전. 기존 행의 host는 NULL.

### Step 5: insert_session()에 host 포함

```rust
// bm25.rs — insert_session()
// INSERT 문에 host 컬럼 추가
rusqlite::params![
    session.id, session.agent.as_str(), session.model, session.project,
    session.cwd, session.git_branch, session.host,  // host 추가
    // ...
]
```

### Step 6: gethostname 의존성 추가

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
gethostname = "0.4"

# crates/secall-core/Cargo.toml
[dependencies]
gethostname.workspace = true
```

## Dependencies

- `gethostname` — 신규 의존성
- 스키마 마이그레이션 포함 (ALTER TABLE)

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. ingest 후 host 필드 확인
cargo run -p secall -- ingest --auto 2>&1 | head -5
# 이후 생성된 MD 파일에서 host 필드 존재 확인:
grep "host:" "$(ls -t ~/Documents/Obsidian\ Vault/seCall/raw/sessions/**/*.md 2>/dev/null | head -1)" && echo "OK: host field present"

# 4. DB에 host 컬럼 존재 확인
sqlite3 ~/.config/secall/secall.db "PRAGMA table_info(sessions)" | grep host && echo "OK: host column exists"
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요.

## Risks

- **기존 세션 호환**: 이미 ingest된 세션의 host는 NULL. 문제 없음 — 필터링 시 NULL은 제외.
- **hostname 변경**: 사용자가 기기 이름을 변경하면 같은 기기의 세션이 다른 host로 기록됨. 실용적으로 드문 케이스.
- **hostname 형식**: macOS는 `MacBook-Pro.local` 같은 형식. 표시 시 `.local` 제거 고려.
- **프라이버시**: hostname이 git 원격에 공유됨. 민감한 경우 config에서 `host_alias` 오버라이드 옵션 제공 고려 (1차에서는 미구현).

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/vault/git.rs` — Task 02 영역
- `crates/secall/src/commands/sync.rs` — Task 03 영역
- `crates/secall/src/commands/reindex.rs` — Task 01 영역
- `crates/secall-core/src/mcp/server.rs` — MCP 필터에 host 추가는 후속 작업
