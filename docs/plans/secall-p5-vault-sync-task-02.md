---
type: task
status: draft
plan: secall-p5-vault-sync
task_number: 2
title: "Vault Git 연동"
parallel_group: A
depends_on: []
updated_at: 2026-04-06
---

# Task 02: Vault Git 연동

## 문제

vault 디렉토리에 git 연동이 없어 기기 간 마크다운 동기화가 수동이다. `secall init` 시 git 초기화 옵션과, vault 변경 시 push/pull 기능이 필요하다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `crates/secall-core/src/vault/config.rs` | 수정 | `VaultConfig`에 `git_remote: Option<String>` 추가 |
| `crates/secall-core/src/vault/git.rs` | 신규 | git 명령 래퍼 (init/pull/push/status) |
| `crates/secall-core/src/vault/mod.rs` | 수정 | `pub mod git;` 추가 |
| `crates/secall/src/commands/init.rs` | 수정 | `--git <remote>` 옵션 추가 |

## Change description

### Step 1: VaultConfig에 git 설정 추가

```rust
// vault/config.rs — VaultConfig
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub path: PathBuf,
    #[serde(default)]
    pub git_remote: Option<String>,  // 추가
}
```

`~/.config/secall/config.toml`:
```toml
[vault]
path = "~/Documents/Obsidian Vault/seCall"
git_remote = "git@github.com:user/secall-vault.git"
```

### Step 2: git 모듈 구현 (vault/git.rs — 신규)

```rust
// vault/git.rs
use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub struct VaultGit<'a> {
    vault_path: &'a Path,
}

impl<'a> VaultGit<'a> {
    pub fn new(vault_path: &'a Path) -> Self {
        Self { vault_path }
    }

    /// vault가 git 저장소인지 확인
    pub fn is_git_repo(&self) -> bool {
        self.vault_path.join(".git").exists()
    }

    /// git init + remote 설정 + .gitignore 생성
    pub fn init(&self, remote: &str) -> Result<()> {
        if self.is_git_repo() {
            tracing::info!("vault is already a git repo");
            return Ok(());
        }

        self.run_git(&["init"])?;
        self.run_git(&["remote", "add", "origin", remote])?;

        // .gitignore — DB, 캐시 파일 제외
        let gitignore = self.vault_path.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, "*.db\n*.db-wal\n*.db-shm\n.DS_Store\n")?;
        }

        // 초기 커밋
        self.run_git(&["add", "."])?;
        self.run_git(&["commit", "-m", "init: seCall vault"])?;

        tracing::info!(remote, "vault git initialized");
        Ok(())
    }

    /// git pull --rebase origin main
    pub fn pull(&self) -> Result<PullResult> {
        if !self.is_git_repo() {
            return Ok(PullResult { new_files: 0, already_up_to_date: true });
        }

        let output = self.run_git(&["pull", "--rebase", "origin", "main"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let already_up_to_date = stdout.contains("Already up to date")
            || stdout.contains("Current branch main is up to date");

        // 새 파일 수 계산 (git diff --stat HEAD@{1} HEAD)
        let new_files = if !already_up_to_date {
            self.run_git(&["diff", "--stat", "HEAD@{1}", "HEAD"])
                .ok()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .filter(|l| l.contains("raw/sessions/"))
                        .count()
                })
                .unwrap_or(0)
        } else {
            0
        };

        Ok(PullResult { new_files, already_up_to_date })
    }

    /// 변경된 파일을 commit + push
    pub fn push(&self, message: &str) -> Result<PushResult> {
        if !self.is_git_repo() {
            return Ok(PushResult { committed: 0 });
        }

        // 변경 감지
        let status = self.run_git(&["status", "--porcelain"])?;
        let changes = String::from_utf8_lossy(&status.stdout);
        if changes.trim().is_empty() {
            return Ok(PushResult { committed: 0 });
        }

        let committed = changes.lines().count();

        self.run_git(&["add", "raw/", "wiki/"])?;
        self.run_git(&["commit", "-m", message])?;
        self.run_git(&["push", "origin", "main"])?;

        tracing::info!(committed, "vault changes pushed");
        Ok(PushResult { committed })
    }

    fn run_git(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.vault_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
        }

        Ok(output)
    }
}

pub struct PullResult {
    pub new_files: usize,
    pub already_up_to_date: bool,
}

pub struct PushResult {
    pub committed: usize,
}
```

### Step 3: secall init --git

```rust
// commands/init.rs — 변경
/// Initialize vault and config
Init {
    #[arg(short, long)]
    vault: Option<PathBuf>,

    /// Git remote URL for vault sync
    #[arg(long)]
    git: Option<String>,
},

// run() 내부에 추가
if let Some(remote) = git {
    let vault_git = VaultGit::new(&config.vault.path);
    vault_git.init(&remote)?;
    // config에 git_remote 저장
    config.vault.git_remote = Some(remote);
    config.save()?;
    println!("Git remote configured. Use `secall sync` to push/pull.");
}
```

### Step 4: .gitignore 기본 내용

```
# seCall vault .gitignore
*.db
*.db-wal
*.db-shm
*.usearch
.DS_Store
.obsidian/
```

> Obsidian 설정 폴더(`.obsidian/`)는 기기별로 다를 수 있으므로 제외.

## Dependencies

- 없음 (git은 시스템 명령 호출)
- P5 Task 01 (reindex)과 독립적으로 구현 가능

## Verification

```bash
# 1. 컴파일 확인
cargo check --all

# 2. 전체 테스트 통과
cargo test --all

# 3. init --git 동작 확인 (임시 디렉토리)
TMPDIR=$(mktemp -d) && \
  cargo run -p secall -- init --vault "$TMPDIR/vault" --git "https://github.com/test/test.git" 2>&1 && \
  test -d "$TMPDIR/vault/.git" && echo "OK: git repo created" && \
  test -f "$TMPDIR/vault/.gitignore" && echo "OK: .gitignore created" && \
  rm -rf "$TMPDIR"

# 4. git 미설치 환경에서 graceful failure 확인 (수동)
# Manual: PATH에서 git 제거 후 secall init --git 실행 → 에러 메시지 확인
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요.

## Risks

- **git 미설치**: 시스템에 git이 없으면 실패. `which git` 사전 체크 후 명확한 에러 메시지 출력.
- **인증 문제**: SSH key 미설정 시 push/pull 실패. 사용자 책임이나, 에러 메시지에 안내 포함.
- **branch 전략**: 1차에서는 `main` 단일 브랜치. 멀티 브랜치 지원은 불필요 (세션이 충돌하지 않으므로).
- **대규모 vault**: 수천 개 MD 파일의 git push가 느릴 수 있음. shallow clone 고려하되 1차에서는 미구현.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/ingest/markdown.rs` — Task 01 영역
- `crates/secall/src/commands/ingest.rs` — Task 03에서 sync로 통합
- `crates/secall-core/src/ingest/types.rs` — Task 04 영역 (host 필드)
