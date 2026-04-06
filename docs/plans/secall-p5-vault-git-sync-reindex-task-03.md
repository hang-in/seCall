---
type: task
status: draft
plan: secall-p5-vault-git-sync-reindex
task_number: 3
title: "secall sync 통합 명령"
parallel_group: B
depends_on: [1, 2]
updated_at: 2026-04-06
---

# Task 03: secall sync 통합 명령

## 문제

vault 동기화를 위해 사용자가 여러 명령을 순서대로 실행해야 한다:
1. git pull (다른 기기 세션 수신)
2. reindex --from-vault (수신된 MD → DB)
3. ingest --auto (로컬 새 세션 → vault)
4. git push (로컬 세션 공유)

이를 하나의 `secall sync` 명령으로 통합한다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall/src/commands/sync.rs` | 신규 | sync 명령 구현 (4-phase) |
| `crates/secall/src/commands/mod.rs` | 수정 | `pub mod sync;` 추가 (Task 01에서 reindex 추가 후) |
| `crates/secall/src/main.rs` | 수정 | `Sync` 서브커맨드 추가 (Commands enum: lines 21-126) |
| `crates/secall/src/commands/ingest.rs` | 수정 | 핵심 로직을 `ingest_sessions()` 함수로 추출 (현재 run(): lines 18-149) |

## Change description

### Step 1: Sync 서브커맨드 정의 (main.rs)

`crates/secall/src/main.rs` Commands enum (lines 21-126)에 추가:

```rust
/// Sync vault with remote (git pull → reindex → ingest → git push)
Sync {
    /// Skip git pull/push (local-only reindex + ingest)
    #[arg(long)]
    local_only: bool,

    /// Dry run — show what would happen without executing
    #[arg(long)]
    dry_run: bool,
},
```

### Step 2: ingest 핵심 로직 함수 분리

`crates/secall/src/commands/ingest.rs` — 기존 run() (lines 18-149)에서 for 루프를 함수로 추출:

```rust
pub struct IngestStats {
    pub ingested: usize,
    pub skipped: usize,
    pub errors: usize,
}

/// ingest 핵심 로직 (run()에서 추출)
pub async fn ingest_sessions(
    config: &Config,
    db: &Database,
    paths: Vec<PathBuf>,
    engine: &SearchEngine,
    vault: &Vault,
    format: &OutputFormat,
) -> Result<IngestStats> {
    // 기존 run()의 for 루프 로직을 여기로 이동
    // ...
}

/// 기존 run() — ingest_sessions() 호출로 대체
pub async fn run(...) -> Result<()> {
    let paths = collect_paths(...)?;
    let stats = ingest_sessions(&config, &db, paths, &engine, &vault, format).await?;
    // Summary 출력
}
```

> **주의**: 기존 `run()` 함수의 동작이 변경되지 않도록 함수 추출만 수행. 반환값과 side effect 동일하게 유지.

### Step 3: sync 명령 구현 (commands/sync.rs — 신규)

```rust
use anyhow::Result;
use secall_core::{
    vault::{Config, Vault, git::VaultGit},
    store::{get_default_db_path, Database},
    ingest::markdown::{parse_session_frontmatter, extract_body_text},
};

pub async fn run(local_only: bool, dry_run: bool) -> Result<()> {
    let config = Config::load_or_default();
    let vault_git = VaultGit::new(&config.vault.path);

    // === Phase 1: Pull (다른 기기 세션 수신) ===
    if !local_only && vault_git.is_git_repo() {
        eprintln!("⟳ Pulling from remote...");
        match vault_git.pull() {
            Ok(result) => {
                if result.already_up_to_date {
                    eprintln!("  Already up to date.");
                } else {
                    eprintln!("  ← {} new session files received.", result.new_files);
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "git pull failed, continuing with local sync");
                eprintln!("  ⚠ Pull failed: {e}");
            }
        }
    }

    // === Phase 2: Reindex (동기화된 MD → DB) ===
    eprintln!("⟳ Reindexing vault...");
    let db = Database::open(&get_default_db_path())?;
    let reindex_result = reindex_vault(&config, &db)?;
    eprintln!(
        "  ⟲ {} new sessions indexed, {} skipped.",
        reindex_result.indexed, reindex_result.skipped
    );

    // === Phase 3: Ingest (로컬 새 세션 → vault) ===
    eprintln!("⟳ Ingesting local sessions...");
    // ingest_sessions() 재사용 (Step 2에서 추출한 함수)
    let ingest_result = run_auto_ingest(&config, &db).await?;
    eprintln!(
        "  → {} ingested, {} skipped, {} errors.",
        ingest_result.ingested, ingest_result.skipped, ingest_result.errors
    );

    // === Phase 4: Push (로컬 세션 공유) ===
    if !local_only && vault_git.is_git_repo() {
        eprintln!("⟳ Pushing to remote...");
        let hostname = gethostname::gethostname()
            .to_string_lossy()
            .to_string();
        let message = format!("sync: {} new sessions from {}", ingest_result.ingested, hostname);

        match vault_git.push(&message) {
            Ok(result) => {
                if result.committed > 0 {
                    eprintln!("  → {} files pushed.", result.committed);
                } else {
                    eprintln!("  No changes to push.");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "git push failed");
                eprintln!("  ⚠ Push failed: {e}");
            }
        }
    }

    eprintln!("\n✓ Sync complete.");
    Ok(())
}

struct ReindexResult {
    indexed: usize,
    skipped: usize,
}

/// vault/raw/sessions/ 스캔 → DB에 없는 MD를 인덱싱
fn reindex_vault(config: &Config, db: &Database) -> Result<ReindexResult> {
    // Task 01의 reindex 로직을 secall-core 내 공유 함수로 호출
    // commands/reindex.rs의 로직과 동일 — 중복 방지를 위해
    // secall-core에 pub fn reindex_from_vault() 추출 필요
    todo!("Task 01 reindex 로직 호출")
}

/// ingest --auto 로직 재사용
async fn run_auto_ingest(config: &Config, db: &Database) -> Result<IngestStats> {
    // Step 2에서 추출한 commands::ingest::ingest_sessions() 호출
    todo!("기존 ingest --auto 로직 재사용")
}
```

> **Developer 참고**: `reindex_vault()`와 `run_auto_ingest()`의 `todo!()`는 실제 구현 시 Task 01/Step 2의 함수를 호출하는 것으로 대체. 코드 중복 대신 함수 재사용.

### Step 4: Claude Code hook 설정 안내

sync 명령이 완성되면 Claude Code에서 자동 실행 가능. `docs/reference/github-vault-sync.md`에 이미 기재:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Initialize",
      "hooks": [{"type": "command", "command": "secall sync --local-only"}]
    }],
    "PostToolUse": [{
      "matcher": "Exit",
      "hooks": [{"type": "command", "command": "secall sync"}]
    }]
  }
}
```

> 세션 시작 시: `sync --local-only` (pull + reindex만)
> 세션 종료 시: `sync` (전체 동기화)

## Dependencies

- **Task 01 (reindex)**: vault → DB 인덱싱 로직 필수
- **Task 02 (git)**: VaultGit pull/push 필수
- `gethostname` — Task 04에서 추가되는 의존성 (커밋 메시지에 호스트명). Task 04가 먼저 완료되지 않으면 `hostname` 문자열 대신 고정값 사용.

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. sync 명령 존재 확인
cargo run -p secall -- sync --help

# 4. local-only 모드 동작 확인 (git 없이)
cargo run -p secall -- sync --local-only 2>&1 | grep "Sync complete"

# 5. ingest 기존 동작 회귀 테스트
cargo run -p secall -- ingest --auto 2>&1

# 6. clippy 통과
cargo clippy --all-targets -- -D warnings
```

## Risks

- **ingest 로직 분리**: 기존 `ingest.rs`의 `run()` (lines 18-149) 함수를 분리하면 ingest 명령 동작에 영향 줄 수 있음. 함수 추출 후 기존 테스트 전체 통과 확인 필수.
- **git push 실패**: 네트워크 문제 등으로 push 실패 시 로컬 ingest는 이미 완료. 다음 sync에서 재시도하면 됨 (idempotent).
- **동시 sync**: 두 기기에서 동시 sync 시 git push conflict 가능. 단, raw/sessions/ 파일은 기기별 유니크이므로 실제 충돌 확률 극히 낮음.
- **reindex 로직 중복**: commands/reindex.rs와 sync.rs에서 동일 로직 사용. secall-core에 공유 함수 추출 필요.

## Scope boundary

수정 금지 파일:
- `crates/secall-core/src/ingest/markdown.rs` — Task 01 영역
- `crates/secall-core/src/vault/git.rs` — Task 02 영역 (호출만)
- `crates/secall-core/src/ingest/types.rs` — Task 04 영역
- `crates/secall-core/src/mcp/server.rs` — MCP 변경 없음
- `crates/secall-core/src/store/db.rs` — Task 01 영역 (호출만)
