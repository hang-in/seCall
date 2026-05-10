---
type: task
plan_slug: p44-wiki-cross-host-sources
task_id: 01
title: secall wiki update 진입 시 auto_commit + pull 자동 호출
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-10
---

# Task 01 — `wiki update` 자동 git pull

## Changed files

수정:
- `crates/secall/src/main.rs:340-362` 의 `WikiAction::Update { ... }` arm 에 `--no-pull` flag 추가 (`#[arg(long)] no_pull: bool`).
- `crates/secall/src/main.rs:597` 부근의 `WikiAction::Update { ... } =>` 분기에서 `no_pull` 을 `commands::wiki::run_update` 로 전달.
- `crates/secall/src/commands/wiki.rs:125-149` 의 `pub async fn run_update` 시그니처에 `no_pull: bool` 추가 + `run_update_with_sink` 로 forward.
- `crates/secall/src/commands/wiki.rs:159-178` 의 `run_update_with_sink` 진입부 (wiki_dir 검증 후, 백엔드 선택 전) 에 `VaultGit::new + check_conflicted_state + auto_commit + pull` 호출 블록 삽입. `dry_run` 또는 `no_pull` true 면 skip.
- `crates/secall-core/src/lib.rs` (필요 시) — `VaultGit` 가 이미 pub re-export 되어 있는지 확인. 안 되어 있으면 추가.

신규:
- 없음.

회귀 테스트:
- 본 task 는 진입부 hook 만 추가 — 기존 테스트가 그대로 통과해야 함. 신규 테스트 X.
- (옵션) `crates/secall/tests/wiki_review_resolve.rs` 와 별도로 `wiki_update_no_pull.rs` 같은 통합 테스트는 task 04 에서 다룸.

## Change description

### 1. CLI flag 추가

```rust
// crates/secall/src/main.rs (WikiAction::Update arm)
#[arg(long)]
review_model: Option<String>,

/// Skip git pull/auto-commit at start (offline / manual sync mode)
#[arg(long)]
no_pull: bool,
```

분기 처리부:

```rust
WikiAction::Update {
    model, backend, since, session, dry_run, review,
    review_backend, review_model, no_pull,
} => {
    commands::wiki::run_update(
        model.as_deref(), backend.as_deref(),
        since.as_deref(), session.as_deref(),
        dry_run, review,
        review_backend.as_deref(), review_model.as_deref(),
        no_pull,
    ).await?;
}
```

### 2. `run_update` / `run_update_with_sink` 시그니처

```rust
#[allow(clippy::too_many_arguments)]
pub async fn run_update(
    model: Option<&str>,
    backend: Option<&str>,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
    review: bool,
    review_backend: Option<&str>,
    review_model: Option<&str>,
    no_pull: bool,
) -> Result<()> {
    run_update_with_sink(
        model, backend, since, session,
        dry_run, review, review_backend, review_model,
        no_pull,
        None,
    ).await.map(|_| ())
}
```

`run_update_with_sink` 에도 동일하게 `no_pull` 추가.

### 3. 진입부 git hook (sync.rs 패턴 차용)

`crates/secall/src/commands/wiki.rs:175-178` 의 `wiki_dir.exists()` 검증 직후 (백엔드 선택 전):

```rust
let vault_git = secall_core::vault::git::VaultGit::new(
    &config.vault.path,
    &config.vault.branch,
);

if vault_git.is_git_repo() {
    if let Some(msg) = vault_git.check_conflicted_state() {
        anyhow::bail!("wiki update aborted — vault git conflict detected.\n\n{msg}");
    }

    if !dry_run && !no_pull {
        match vault_git.auto_commit() {
            Ok(true) => eprintln!("Auto-committed unstaged vault changes before pull."),
            Ok(false) => {}
            Err(e) => eprintln!("Warning: auto-commit failed: {e}"),
        }
        match vault_git.pull() {
            Ok(result) if result.already_up_to_date => {}
            Ok(result) => eprintln!("Pulled vault: {} new session file(s).", result.new_files),
            Err(e) => eprintln!("Warning: vault pull failed: {e}"),
        }
    }
}
```

> `sync.rs:83-145` 동일 패턴. `dry_run` 또는 `--no-pull` 시 skip.
> Pull 실패는 `Warning` 로 격하 — wiki update 자체는 계속 진행 (오프라인 fallback).
> Conflict 상태는 `anyhow::bail!` — task 03 의 marker 자동 resolve 와는 별개. 본 task 는 미해결 충돌이 있으면 abort.

### 4. `VaultGit` re-export 확인

`crates/secall-core/src/lib.rs` 에서 `pub mod vault;` + `vault::git::VaultGit` 경로가 secall (binary crate) 에서 접근 가능해야 함. `sync.rs:7` 에서 이미 `vault::{git::VaultGit, ...}` import 되어 있으므로 OK — 추가 작업 없을 가능성 높음. 확인만.

## Dependencies

- 의존 task 없음 (parallel_group A 시작).
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall
cargo clippy --all-targets -p secall

# 2. CLI help — flag 등록 확인
./target/debug/secall wiki update --help | grep -E "no-pull"

# 3. 기존 wiki 통합 테스트 회귀
cargo test -p secall --test wiki_review_resolve

# 4. dry-run 시 git 호출 안 함 (수동 — 임시 vault 에서)
# (수동) cd /tmp/test-vault && secall wiki update --dry-run --no-pull
# git fetch 가 stdout 에 안 나와야 함

# 5. 정상 모드 동작 (수동, 실제 vault)
# (수동) secall wiki update
# stdout 에 "Pulled vault: ..." 또는 "Auto-committed ..." 가 나오거나 silent (already up to date).
```

## Risks

- **dry-run 과 no-pull 의 분리** — `dry_run` 은 LLM 호출까지 skip, `no_pull` 은 git ops 만 skip. 둘 다 true 면 git+LLM 모두 skip. 둘 다 false 면 정상.
- **sync 와의 중복 pull** — 사용자가 `secall sync && secall wiki update` 호출하면 pull 이 두 번 — 두 번째는 already-up-to-date 라 비용 거의 0. 정합성 OK.
- **auto-commit 의 부작용** — vault 안의 임의 변경이 자동으로 커밋됨. sync.rs 와 동일 동작 — 사용자가 인지하고 사용 중. 별도 안내 불필요.
- **git lock 충돌** — sync 와 wiki update 가 동시 실행되면 `.git/index.lock` 충돌. 본 plan 은 직렬 실행 가정. 동시 실행 차단은 별도 plan.
- **`is_git_repo()` 가 false 인 vault** — git ops skip. 이 경우 cross-host 시나리오 자체가 성립 안 하므로 OK.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/lint.rs` — task 02 영역.
- `crates/secall-core/src/vault/git.rs` — task 03 의 helper 추가 영역. 본 task 는 기존 함수 호출만.
- `crates/secall/src/commands/sync.rs` — pull 패턴의 reference. 변경 X.
- `crates/secall/src/commands/wiki.rs` 의 `run_review` / `build_reviewer` / `resolve_review_backend` — P43 영역. 본 task 는 진입부 hook 만.
- `web/` — 본 plan 의 non-goal.
- README / docs — task 04 영역.
