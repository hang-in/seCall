---
type: task
status: pending
updated_at: 2026-04-17
plan: p30-openai-sync-no-semantic-34-35
task_number: 2
parallel_group: A
depends_on: []
github_issue: "#34"
---

# Task 02 — sync --no-semantic 플래그 추가

## Changed files

- `crates/secall/src/main.rs:183-195` — `Sync` 커맨드 구조체에 `no_semantic` 필드 추가
- `crates/secall/src/main.rs:456-461` — 디스패치에서 `no_semantic` 인자 전달
- `crates/secall/src/commands/sync.rs:14` — `run` 함수 시그니처에 `no_semantic: bool` 추가
- `crates/secall/src/commands/sync.rs:293` — 하드코딩 `false` → `no_semantic` 변수로 교체

## Change description

### Step 1: CLI 인자 추가 (`main.rs:183-195`)

`Sync` 구조체의 `no_wiki` 필드(L193-194) 뒤에 추가:

```rust
/// Skip semantic edge extraction during ingest
#[arg(long)]
no_semantic: bool,
```

### Step 2: 디스패치 업데이트 (`main.rs:456-461`)

현재:
```rust
Commands::Sync {
    local_only,
    dry_run,
    no_wiki,
} => {
    commands::sync::run(local_only, dry_run, no_wiki).await?;
}
```

변경:
```rust
Commands::Sync {
    local_only,
    dry_run,
    no_wiki,
    no_semantic,
} => {
    commands::sync::run(local_only, dry_run, no_wiki, no_semantic).await?;
}
```

### Step 3: `run` 함수 시그니처 변경 (`sync.rs:14`)

현재:
```rust
pub async fn run(local_only: bool, dry_run: bool, no_wiki: bool) -> Result<()> {
```

변경:
```rust
pub async fn run(local_only: bool, dry_run: bool, no_wiki: bool, no_semantic: bool) -> Result<()> {
```

### Step 4: 하드코딩 제거 (`sync.rs:293`)

현재:
```rust
false, // no_semantic: sync에서는 시맨틱 추출 활성화
```

변경:
```rust
no_semantic,
```

## Dependencies

없음 (독립 태스크)

## Verification

```bash
# 1. 타입 체크
cargo check -p secall

# 2. CLI 도움말에 --no-semantic 표시 확인
cargo run -- sync --help 2>&1 | grep -i "no-semantic"

# 3. 기존 테스트 통과 (sync 관련)
cargo test -p secall
```

## Risks

- **매우 낮음**: 함수 시그니처에 `bool` 하나 추가 + 하드코딩 제거. `run` 호출부는 `main.rs` 한 곳뿐 (grep으로 확인됨)
- `ingest_sessions`의 `no_semantic` 파라미터 위치가 8번째 인자(0-based: 7)인데, 기존 `false` 자리를 변수로 교체하므로 위치 오류 가능성 없음

## Scope boundary (수정 금지)

- `crates/secall-core/src/graph/semantic.rs` — Task 01 영역
- `crates/secall-core/src/vault/config.rs` — Task 01 영역
- `crates/secall/src/commands/ingest.rs` — 기존 `--no-semantic` 구현 건드리지 않음
