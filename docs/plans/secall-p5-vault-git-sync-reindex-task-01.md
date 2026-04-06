---
type: task
status: draft
plan: secall-p5-vault-git-sync-reindex
task_number: 1
title: "MD → DB 역인덱싱 (reindex --from-vault)"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 01: MD → DB 역인덱싱 (reindex --from-vault)

## 문제

vault에 마크다운 파일이 있지만 DB에는 해당 세션이 없는 경우가 발생한다:
- 다른 기기에서 동기화된 세션 (git pull 후)
- DB 손실/재생성 후 복구
- 수동으로 vault에 복사된 MD 파일

현재 `secall ingest`는 JSONL/JSON 원본만 파싱 가능하고, 마크다운에서 역으로 DB를 재구축하는 기능이 없다.

### 현재 코드

```rust
// lint.rs:401-419 — session_id만 추출하는 코드가 존재
fn extract_session_id_from_frontmatter(content: &str) -> Option<String>
// → frontmatter 전체 파싱으로 확장 필요
```

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/ingest/markdown.rs` | 수정 | `parse_session_frontmatter()`, `extract_body_text()` 함수 추가 (현재 `render_session()` at line 8만 존재) |
| `crates/secall-core/src/store/db.rs` | 수정 | `insert_session_from_vault()` 메서드 추가 (현재 `session_exists()` at lines 278-285 활용) |
| `crates/secall/src/commands/reindex.rs` | 신규 | `reindex --from-vault` 명령 구현 |
| `crates/secall/src/commands/mod.rs` | 수정 | `pub mod reindex;` 추가 (현재 10개 모듈: lines 1-10) |
| `crates/secall/src/main.rs` | 수정 | `Reindex` 서브커맨드 추가 (Commands enum: lines 21-126) |
| `crates/secall-core/Cargo.toml` | 수정 | `serde_yaml` workspace 의존성 추가 |
| `Cargo.toml` (workspace) | 수정 | `[workspace.dependencies]`에 `serde_yaml = "0.9"` 추가 |

## Change description

### Step 1: serde_yaml 의존성 추가

```toml
# Cargo.toml (workspace root)
[workspace.dependencies]
serde_yaml = "0.9"

# crates/secall-core/Cargo.toml
[dependencies]
serde_yaml.workspace = true
```

> 현재 워크스페이스에 serde_yaml 없음. frontmatter YAML 파싱에 필요.

### Step 2: frontmatter 파싱 (markdown.rs)

`crates/secall-core/src/ingest/markdown.rs`에 기존 `render_session()` (line 8) 아래에 역파싱 함수 추가:

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(default)]
pub struct SessionFrontmatter {
    pub session_id: String,
    pub agent: String,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<String>,
    pub date: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub turns: Option<u32>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub tools_used: Option<Vec<String>>,
    pub host: Option<String>,  // Task 04에서 추가되는 필드 — #[serde(default)]로 호환
    pub status: Option<String>,
}

/// vault 마크다운 파일에서 frontmatter YAML을 파싱.
pub fn parse_session_frontmatter(content: &str) -> crate::error::Result<SessionFrontmatter> {
    let fm = content
        .strip_prefix("---\n")
        .and_then(|s| s.split_once("\n---"))
        .map(|(fm, _)| fm)
        .ok_or_else(|| crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: anyhow::anyhow!("no frontmatter found"),
        })?;

    let parsed: SessionFrontmatter = serde_yaml::from_str(fm)
        .map_err(|e| crate::SecallError::Parse {
            path: "<frontmatter>".to_string(),
            source: e.into(),
        })?;
    Ok(parsed)
}

/// frontmatter 이후의 본문 텍스트 추출 (턴 내용).
pub fn extract_body_text(content: &str) -> String {
    content
        .split_once("\n---\n")
        .map(|(_, body)| {
            body.split_once('\n')
                .map(|(_, rest)| rest)
                .unwrap_or(body)
        })
        .unwrap_or("")
        .to_string()
}
```

> P4 typed error(`SecallError::Parse`)를 사용. `#[serde(default)]`로 필드 추가 시 하위 호환.

### Step 3: DB에 vault MD 기반 insert (db.rs)

`crates/secall-core/src/store/db.rs`에 추가. 기존 `session_exists()` (lines 278-285) 활용:

```rust
impl Database {
    /// vault 마크다운의 frontmatter로 sessions 테이블에 insert.
    /// turns 테이블에는 본문 전체를 단일 FTS 청크로 저장 (원본 턴 경계 복원 불가).
    pub fn insert_session_from_vault(
        &self,
        fm: &SessionFrontmatter,
        body_text: &str,
        vault_path: &str,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO sessions(
                id, agent, model, project, cwd, git_branch,
                start_time, end_time, turn_count, tokens_in, tokens_out,
                tools_used, vault_path, ingested_at, status
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, NULL,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, datetime('now'), 'reindexed'
            )",
            rusqlite::params![
                fm.session_id, fm.agent, fm.model, fm.project, fm.cwd,
                fm.start_time, fm.end_time, fm.turns.unwrap_or(0),
                fm.tokens_in.unwrap_or(0), fm.tokens_out.unwrap_or(0),
                fm.tools_used.as_ref().map(|t| t.join(",")),
                vault_path,
            ],
        )?;

        // FTS 인덱싱 — 본문 전체를 하나의 청크로
        if !body_text.trim().is_empty() {
            self.conn().execute(
                "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, 0)",
                rusqlite::params![body_text, fm.session_id],
            )?;
        }

        Ok(())
    }
}
```

> `INSERT OR IGNORE`로 중복 세션 자동 skip. `status = 'reindexed'`로 원본 ingest와 구분.

### Step 4: reindex 서브커맨드 (commands/reindex.rs — 신규)

```rust
pub fn run(from_vault: bool) -> Result<()> {
    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    if !from_vault {
        anyhow::bail!("--from-vault flag is required");
    }

    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        println!("No vault sessions directory found.");
        return Ok(());
    }

    let mut indexed = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to read");
                errors += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
                errors += 1;
                continue;
            }
        };

        // 중복 체크 — db.session_exists() (db.rs:278)
        match db.session_exists(&fm.session_id) {
            Ok(true) => { skipped += 1; continue; }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "DB check failed");
                errors += 1;
                continue;
            }
        }

        let vault_path = path
            .strip_prefix(&config.vault.path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let body = extract_body_text(&content);

        match db.insert_session_from_vault(&fm, &body, &vault_path) {
            Ok(()) => indexed += 1,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "reindex failed");
                errors += 1;
            }
        }
    }

    eprintln!(
        "\nReindex: {} indexed, {} skipped (duplicate), {} errors",
        indexed, skipped, errors
    );
    Ok(())
}
```

### Step 5: main.rs에 서브커맨드 추가

`crates/secall/src/main.rs` Commands enum (lines 21-126)에 추가:

```rust
/// Rebuild DB index from vault markdown files
Reindex {
    /// Rebuild from vault markdown files
    #[arg(long)]
    from_vault: bool,
},
```

`crates/secall/src/commands/mod.rs`에 추가:

```rust
pub mod reindex;
```

## Dependencies

- `serde_yaml = "0.9"` — 신규 의존성 (frontmatter YAML 파싱)
- `walkdir` — 이미 의존성에 포함 (workspace)
- P4 SecallError — `SecallError::Parse` variant 사용

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. reindex 기능 테스트 (실제 vault 사용)
cargo run -p secall -- reindex --from-vault 2>&1 | grep "Reindex:"

# 4. reindex 후 검색 가능 확인
cargo run -p secall -- recall "test" --limit 3

# 5. 중복 실행 시 모두 skip되는지 확인
cargo run -p secall -- reindex --from-vault 2>&1 | grep "skipped"

# 6. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **턴 경계 복원 불가**: 마크다운에서 역파싱 시 원본 턴 구분이 어려움. 본문 전체를 하나의 FTS 청크로 인덱싱. BM25 검색은 정상 동작하나, turn_index 기반 `secall get <id>:<turn>` 조회는 제한적.
- **serde_yaml 의존성**: 신규 추가. `serde_yaml = "0.9"` 권장 (0.8은 deprecated).
- **frontmatter 스키마 변경**: 향후 필드 추가 시 `SessionFrontmatter` 업데이트 필요. `#[serde(default)]`로 하위 호환성 확보.
- **벡터 인덱싱 미포함**: reindex는 FTS만 재구축. 벡터 임베딩은 `secall embed`로 별도 실행 필요.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/vault/mod.rs` — vault 쓰기 로직 변경 없음
- `crates/secall-core/src/ingest/claude.rs` — JSONL 파서 변경 없음
- `crates/secall-core/src/ingest/codex.rs` — JSONL 파서 변경 없음
- `crates/secall-core/src/ingest/gemini.rs` — JSON 파서 변경 없음
- `crates/secall-core/src/search/vector.rs` — 벡터 인덱싱은 별도
- `crates/secall-core/src/hooks/mod.rs` — hook 변경 없음
- `crates/secall-core/src/ingest/types.rs` — Session 구조체 수정은 Task 04 영역
