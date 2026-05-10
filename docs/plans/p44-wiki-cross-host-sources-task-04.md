---
type: task
plan_slug: p44-wiki-cross-host-sources
task_id: 04
title: 회귀 테스트 + 문서
parallel_group: C
depends_on: [01, 02, 03]
status: pending
updated_at: 2026-05-10
---

# Task 04 — Cross-host 시나리오 통합 테스트 + 문서

## Changed files

수정:
- `README.md` — Wiki 섹션에 cross-host 동기화 절 + `--no-pull` 옵션 안내. Updates 표 갱신 (`v0.10.1` 또는 `v0.11.0`).
- `README.en.md` — 동일 변경 영문 미러.
- `docs/reference/llm-config.md` — 본 task 영역 외 (LLM 설정 무관). 변경 X.

신규:
- `crates/secall-core/tests/wiki_cross_host_resolve.rs` (신규 통합 테스트) — tempfile 기반 mini git vault 에서:
  1. 두 host 시나리오 (다른 sources) → conflict 생성 → `unmerged_files` / `extract_sources_from_conflicted` 가 합집합 반환 검증.
  2. `format_with_frontmatter` + sources 합집합 → 결과 wiki 파일이 양쪽 sources 모두 포함 검증.
  3. `finish_conflict_resolution` 가 working tree clean 으로 만드는지 검증.
- `docs/community/v0.10.1-release-notes.md` (신규, 또는 v0.11.0) — P44 변경 highlight.

회귀 테스트:
- 위 신규 통합 테스트가 본 task 의 핵심 검증.

## Change description

### 1. 통합 테스트 (`tests/wiki_cross_host_resolve.rs`)

```rust
use std::process::Command;
use tempfile::tempdir;

use secall_core::vault::git::VaultGit;

fn init_vault_repo(dir: &std::path::Path) {
    Command::new("git").args(["init", "-b", "main"]).current_dir(dir).status().unwrap();
    Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(dir).status().unwrap();
    Command::new("git").args(["config", "user.name", "Test"]).current_dir(dir).status().unwrap();
    std::fs::create_dir_all(dir.join("wiki")).unwrap();
}

fn commit_all(dir: &std::path::Path, message: &str) {
    Command::new("git").args(["add", "."]).current_dir(dir).status().unwrap();
    Command::new("git").args(["commit", "-m", message]).current_dir(dir).status().unwrap();
}

#[test]
fn cross_host_conflict_extracts_sources_union() {
    let dir = tempdir().unwrap();
    init_vault_repo(dir.path());

    // ours: sources sess-A
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-A\n---\n\nours body",
    )
    .unwrap();
    commit_all(dir.path(), "ours");

    // branch incoming: sources sess-B
    Command::new("git").args(["checkout", "-b", "incoming"]).current_dir(dir.path()).status().unwrap();
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-B\n---\n\ntheirs body",
    )
    .unwrap();
    commit_all(dir.path(), "theirs");

    // back to main + merge → conflict
    Command::new("git").args(["checkout", "main"]).current_dir(dir.path()).status().unwrap();
    let merge = Command::new("git").args(["merge", "incoming"]).current_dir(dir.path()).status().unwrap();
    assert!(!merge.success(), "expected merge conflict");

    let vault_git = VaultGit::new(dir.path(), "main");
    let unmerged = vault_git.unmerged_files().unwrap();
    assert!(unmerged.iter().any(|p| p == "wiki/topic.md"));

    let sources = vault_git.extract_sources_from_conflicted("wiki/topic.md").unwrap();
    assert!(sources.contains(&"sess-A".to_string()));
    assert!(sources.contains(&"sess-B".to_string()));
}

#[test]
fn finish_conflict_resolution_clears_merge_state() {
    let dir = tempdir().unwrap();
    init_vault_repo(dir.path());

    std::fs::write(dir.path().join("wiki/x.md"), "---\nsources:\n  - A\n---\nours").unwrap();
    commit_all(dir.path(), "ours");

    Command::new("git").args(["checkout", "-b", "incoming"]).current_dir(dir.path()).status().unwrap();
    std::fs::write(dir.path().join("wiki/x.md"), "---\nsources:\n  - B\n---\ntheirs").unwrap();
    commit_all(dir.path(), "theirs");

    Command::new("git").args(["checkout", "main"]).current_dir(dir.path()).status().unwrap();
    let _ = Command::new("git").args(["merge", "incoming"]).current_dir(dir.path()).status();

    let vault_git = VaultGit::new(dir.path(), "main");

    // 충돌 해결: 양쪽 sources union 포함 단일 파일 작성
    std::fs::write(
        dir.path().join("wiki/x.md"),
        "---\ntype: topic\nsources:\n  - A\n  - B\n---\n\nresolved",
    )
    .unwrap();
    vault_git.stage_resolved("wiki/x.md").unwrap();
    vault_git.finish_conflict_resolution("auto: resolve").unwrap();

    // working tree clean 확인
    let status = Command::new("git").args(["status", "--porcelain"]).current_dir(dir.path()).output().unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty(), "tree should be clean");
}
```

> **plain merge 만 검증** — rebase 모드의 stage 2/3 검증은 더 복잡한 setup 필요. 본 task 에선 plain merge 시나리오로 충분 (rebase 도 동일 stage 의미).
> CI 환경의 git 버전 호환성: `git -b main` flag 는 git 2.28+. 워크스페이스 가정.

### 2. README 변경 (한/영)

`README.md` 의 wiki 섹션에 추가 절:

```markdown
### Cross-host 동기화 (다중 머신 vault)

윈도우/맥 등 여러 머신에서 같은 vault 를 사용할 때, `secall wiki update` 는 시작 시 자동으로 `git pull --rebase` 를 시도합니다 (P44).

| 시나리오 | 동작 |
|---|---|
| 같은 토픽 wiki 가 양쪽에서 갱신됨 | 충돌 감지 → 양쪽 `sources` 합집합으로 해당 토픽 자동 재생성 + commit |
| wiki 외 파일 (raw/sessions/, log/) 에 충돌 | abort + 사용자 안내 |
| 오프라인 환경 | `--no-pull` flag 로 git ops skip |
| dry-run | git ops 자체 skip (LLM 호출도 skip) |

```bash
# 정상 사용 (자동 pull + 충돌 자동 resolve)
secall wiki update

# 오프라인 모드
secall wiki update --no-pull
```

기존 동작과의 차이 (P44):
- `merge_with_existing` 의 본문 누적이 제거됨 — 같은 토픽 재호출 시 본문은 새로 생성된 것 단일. `sources` 만 합집합 보존.
```

`README.en.md` 동일 절 영문 미러.

Updates 표:

```
| 2026-05-10 | v0.10.1 | P44 Wiki cross-host 머지: `wiki update` 진입 시 자동 git pull, 충돌 감지 시 양쪽 sources 합집합으로 자동 재생성, `--no-pull` flag, `merge_with_existing` 본문 누적 제거 |
```

### 3. release-notes (신규)

`docs/community/v0.10.1-release-notes.md`:

```markdown
---
type: release-notes
version: v0.10.1
date: 2026-05-10
---

## v0.10.1 (P44)

- `secall wiki update` 가 시작 시 자동으로 vault git pull 을 시도. 다중 머신 (윈도우/맥) 사용 시 cross-host 동기화 자동화.
- 같은 토픽의 wiki 가 양쪽 머신에서 갱신되어 git 충돌이 발생하면, 양쪽의 `sources: Vec<String>` 합집합으로 해당 토픽을 자동 재생성하여 충돌 해결.
- `--no-pull` flag — 오프라인 환경에서 git ops skip.
- `merge_with_existing()` 의 본문 단순 concat 제거 — 같은 토픽 재호출 시 본문 누적 없음.
- 영향 범위: wiki 만. 일기/raw 세션/graph 파일은 무관.
```

### 4. CLAUDE.md 업데이트 (선택)

`/Users/d9ng/privateProject/seCall/CLAUDE.md` 의 Completed 섹션에 P44 추가 — 본 task 의 verification 명령엔 포함 X (별도 manual update).

## Dependencies

- task 01 (auto pull) — README 의 `--no-pull` flag 안내 정합성.
- task 02 (merge_with_existing 본문 정리) — README 의 "본문 누적 제거" 안내 정합성.
- task 03 (conflict marker resolution) — 통합 테스트의 `unmerged_files` / `extract_sources_from_conflicted` / `finish_conflict_resolution` helper 가 task 03 에서 추가됨.
- crate dep: `tempfile` 은 이미 `[dev-dependencies]` 에 있음. 추가 X.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo clippy --all-targets -p secall-core

# 2. 통합 테스트
cargo test -p secall-core --test wiki_cross_host_resolve

# 3. README 변경 grep
grep -c "no-pull" README.md           # 1 이상
grep -c "no-pull" README.en.md        # 1 이상
grep -c "Cross-host" README.md        # 1 이상
grep "v0.10.1" README.md              # Updates 표

# 4. release notes 존재
ls docs/community/v0.10.1-release-notes.md

# 5. (수동) markdown lint (있으면)
markdownlint README.md README.en.md docs/community/v0.10.1-release-notes.md
```

## Risks

- **CI 환경의 git 사용자 미설정** — 통합 테스트가 `git config user.email/name` 를 vault 디렉터리 단위로 설정하므로 CI runner 의 global config 에 의존 안 함. 안전.
- **CI 환경의 git 버전** — `git init -b main` 은 git 2.28+. workspace 가정. 미만 버전이면 `git init && git symbolic-ref HEAD refs/heads/main` 으로 fallback.
- **테스트 격리** — tempfile 기반이라 병렬 실행 시 충돌 없음. `cargo test` 의 default 병렬 OK.
- **Windows path separator** — `wiki/topic.md` (POSIX) — Rust `Path` 가 normalize. 하지만 git output 의 `wiki/topic.md` 는 Windows 에서도 forward-slash 반환 (git 의 정책). 안전.
- **CLAUDE.md 갱신 누락** — 본 task verification 에 포함 X. 사용자가 별도 처리.
- **release notes 의 버전 번호 (v0.10.1 vs v0.11.0)** — semantic 우선순위. P44 는 backwards-compatible additive 기능 → v0.10.1. minor bump 라면 v0.11.0. 본 plan 은 v0.10.1 으로 통일.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/lint.rs` — task 02 영역.
- `crates/secall-core/src/vault/git.rs` — task 03 영역.
- `crates/secall/src/commands/wiki.rs` — task 01 / 03 영역.
- `crates/secall/src/main.rs` — task 01 영역.
- `docs/reference/llm-config.md` — 본 task 영역 외 (LLM 설정 무관).
- `docs/plans/p44-*.md` — 본 plan 의 다른 task 문서. 본 task 가 그 문서를 손대지 않음.
- 다른 plan 의 docs (`docs/plans/p43-*` 등) — 영역 외.
