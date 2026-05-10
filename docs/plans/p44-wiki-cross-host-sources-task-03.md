---
type: task
plan_slug: p44-wiki-cross-host-sources
task_id: 03
title: Pull 후 wiki conflict marker 감지 → sources 합집합 재생성
parallel_group: B
depends_on: [01, 02]
status: pending
updated_at: 2026-05-10
---

# Task 03 — Cross-host conflict 자동 resolve

## Changed files

수정:
- `crates/secall-core/src/vault/git.rs` — 신규 helper 3개 추가:
  - `pub fn unmerged_files(&self) -> Result<Vec<String>>` — `git diff --name-only --diff-filter=U` 출력 파싱.
  - `pub fn extract_sources_from_conflicted(&self, path: &str) -> Result<Vec<String>>` — 충돌 파일을 양쪽 (HEAD + MERGE_HEAD or rebase의 stage 2/3) 에서 읽어 frontmatter 의 `sources` 합집합 반환.
  - `pub fn stage_resolved(&self, path: &str) -> Result<()>` — `git add <path>`.
- `crates/secall/src/commands/wiki.rs:159-178` 의 `run_update_with_sink` 진입부 (task 01 의 pull 호출 직후) 에 conflict-resolve 블록 삽입.

신규:
- `crates/secall/src/commands/wiki.rs` 안에 `async fn auto_resolve_wiki_conflicts(config, vault_git, db, reviewer, ...) -> Result<usize>` helper. 충돌 wiki 파일별로:
  1. `extract_sources_from_conflicted` 로 양쪽 sources 합집합 획득
  2. 합집합 sessions 를 DB 에서 fetch (`Database::sessions_by_ids` 사용)
  3. `build_wiki_page` (또는 동등한 backend 호출) 로 새 본문 생성
  4. `validate_frontmatter` + `merge_with_existing` (task 02 의 새 동작) 으로 frontmatter 합성
  5. 충돌 파일에 새 내용 쓰기 + `stage_resolved`
  6. 모든 충돌 처리 후 `git rebase --continue` (rebase 모드면) 또는 `git commit -m "auto-resolve wiki conflicts"` (merge 모드면).

회귀 테스트:
- `crates/secall-core/src/vault/git.rs` 의 `#[cfg(test)] mod tests` 에 helper 단위 테스트 2개:
  1. `unmerged_files_returns_only_wiki_paths` — mock vault 에서 `git diff --name-only --diff-filter=U` 가 wiki 외 파일도 반환하면 호출자가 필터링하도록 — 본 helper 는 모든 unmerged 반환만.
  2. `extract_sources_from_conflicted_returns_union` — `git show :2:wiki/x.md` / `:3:wiki/x.md` 가 다른 sources 를 가진 시나리오에서 합집합 반환.
- 통합 테스트 (`crates/secall/tests/wiki_cross_host_resolve.rs` 신규) — task 04 에서 다룸. 본 task 는 단위 helper 만.

## Change description

### 1. VaultGit helper 추가

```rust
// crates/secall-core/src/vault/git.rs
impl<'a> VaultGit<'a> {
    /// 충돌 (unmerged) 상태인 파일 경로 목록.
    pub fn unmerged_files(&self) -> crate::error::Result<Vec<String>> {
        if !self.is_git_repo() {
            return Ok(vec![]);
        }
        let output = self.run_git(&["diff", "--name-only", "--diff-filter=U"])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    /// 충돌 파일의 양쪽 stage 에서 frontmatter 의 sources 합집합 반환.
    /// stage 2 = "ours" (HEAD), stage 3 = "theirs" (MERGE_HEAD or incoming rebase).
    pub fn extract_sources_from_conflicted(
        &self,
        path: &str,
    ) -> crate::error::Result<Vec<String>> {
        let mut sources: Vec<String> = Vec::new();
        for stage in ["2", "3"] {
            let spec = format!(":{stage}:{path}");
            let output = match self.run_git(&["show", &spec]) {
                Ok(o) => o,
                // stage 2 또는 3 가 없는 경우 (e.g. add/add conflict) skip.
                Err(_) => continue,
            };
            let content = String::from_utf8_lossy(&output.stdout);
            for sid in parse_sources_from_frontmatter(&content) {
                if !sources.contains(&sid) {
                    sources.push(sid);
                }
            }
        }
        Ok(sources)
    }

    /// `git add <path>` — conflict resolve 후 staging.
    pub fn stage_resolved(&self, path: &str) -> crate::error::Result<()> {
        self.run_git(&["add", path])?;
        Ok(())
    }

    /// rebase 모드면 `git rebase --continue`, merge 모드면 `git commit`.
    pub fn finish_conflict_resolution(&self, message: &str) -> crate::error::Result<()> {
        let git_dir = self.vault_path.join(".git");
        if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            // GIT_EDITOR=true 로 commit message 편집 skip.
            let mut cmd = std::process::Command::new("git");
            cmd.args(["rebase", "--continue"])
                .env("GIT_EDITOR", "true")
                .current_dir(self.vault_path);
            let output = cmd.output()?;
            if !output.status.success() {
                return Err(crate::SecallError::Config(format!(
                    "git rebase --continue failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
        } else if git_dir.join("MERGE_HEAD").exists() {
            self.run_git(&["commit", "-m", message])?;
        }
        Ok(())
    }
}

/// 단순 frontmatter 파서 — `sources:` 다음의 `  - <id>` 블록 추출.
/// `secall_core::wiki::lint::parse_frontmatter_fields` 와 동등 동작이지만
/// vault::git 의 dependency 사이클 (lint → git → lint) 회피를 위해 자체 구현.
fn parse_sources_from_frontmatter(content: &str) -> Vec<String> {
    let Some(rest) = content.strip_prefix("---\n") else { return vec![]; };
    let Some(end) = rest.find("\n---") else { return vec![]; };
    let fm = &rest[..end];

    let mut in_sources = false;
    let mut out = Vec::new();
    for line in fm.lines() {
        if line.starts_with("sources:") {
            in_sources = true;
            continue;
        }
        if in_sources {
            if let Some(stripped) = line.strip_prefix("  - ") {
                out.push(stripped.trim().to_string());
            } else if !line.starts_with(' ') && !line.is_empty() {
                in_sources = false;
            }
        }
    }
    out
}
```

> **순환 의존 회피**: `vault::git` 모듈이 `wiki::lint` 의 `parse_frontmatter_fields` 를 import 하면 `wiki::lint` → `vault::git` → `wiki::lint` 사이클 위험. 별도 simplified parser 사용. 정합성은 task 04 의 통합 테스트에서 검증.

### 2. wiki.rs 의 conflict-resolve 블록

task 01 의 pull 호출 직후 (성공 시):

```rust
// task 01 의 pull 블록 끝 직후
if vault_git.is_git_repo() && !dry_run && !no_pull {
    let unmerged = vault_git.unmerged_files().unwrap_or_default();
    let wiki_conflicts: Vec<String> = unmerged
        .into_iter()
        .filter(|p| p.starts_with("wiki/") && p.ends_with(".md"))
        .collect();

    let non_wiki_conflicts: Vec<String> = vault_git
        .unmerged_files()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !(p.starts_with("wiki/") && p.ends_with(".md")))
        .collect();

    if !non_wiki_conflicts.is_empty() {
        anyhow::bail!(
            "wiki update aborted — non-wiki conflicts pending:\n{}\n\
             Resolve manually then re-run.",
            non_wiki_conflicts.join("\n")
        );
    }

    if !wiki_conflicts.is_empty() {
        eprintln!(
            "Auto-resolving {} wiki conflict(s) via sources union regeneration...",
            wiki_conflicts.len()
        );
        let resolved = auto_resolve_wiki_conflicts(
            &config,
            &vault_git,
            &wiki_conflicts,
        )
        .await?;
        eprintln!("Resolved {resolved} wiki conflict(s).");
    }
}
```

`auto_resolve_wiki_conflicts` 본문 (개략):

```rust
async fn auto_resolve_wiki_conflicts(
    config: &Config,
    vault_git: &VaultGit<'_>,
    paths: &[String],
) -> Result<usize> {
    use secall_core::store::Database;

    let db = Database::open(&config.vault.path)?;
    let backend_name = config.wiki.default_backend.clone();
    let resolved_model = resolve_backend_model(config, &backend_name, None);
    let backend = build_wiki_backend(config, &backend_name, &resolved_model)?;

    let wiki_dir = config.vault.path.join("wiki");
    let mut count = 0usize;

    for path in paths {
        let sources = vault_git.extract_sources_from_conflicted(path)?;
        if sources.is_empty() {
            eprintln!("  Skip {path} — no sources in either side.");
            continue;
        }

        let sessions = db.sessions_by_ids(&sources)?;
        if sessions.is_empty() {
            eprintln!("  Skip {path} — sessions not found locally.");
            continue;
        }

        // 기존 generate_wiki_page 흐름 재사용 — prompt 빌드 + backend.generate
        let prompt = build_prompt_from_sessions(&sessions);
        let raw = backend.generate(&prompt).await?;
        let validated = secall_core::wiki::lint::validate_frontmatter(&raw, &sources);

        // task 02 의 새 merge_with_existing — 본문은 새 내용으로 교체.
        // 단, 충돌 상태에선 working tree 의 path 가 conflict marker 포함이라 read 불가.
        // 대신 `stage 2 (ours)` 의 frontmatter 만 보존하기 위해 별도 처리:
        // → 단순화: validated 를 그대로 file 에 쓰고, sources 는 합집합으로 frontmatter 보정.
        let final_content = secall_core::wiki::lint::format_resolved(
            &validated,
            &sources,
        );

        let full_path = wiki_dir.join(path.trim_start_matches("wiki/"));
        std::fs::write(&full_path, &final_content)?;
        vault_git.stage_resolved(path)?;
        count += 1;
        eprintln!("  Resolved: {path} (sources: {})", sources.len());
    }

    if count > 0 {
        vault_git.finish_conflict_resolution("auto: resolve wiki conflicts via sources union")?;
    }
    Ok(count)
}
```

> **`format_resolved`**: task 02 의 `merge_with_existing` 는 working-tree 의 기존 파일을 읽음 — 충돌 상태에선 marker 가 섞여 있어 사용 불가. 대신 `lint.rs` 에 `pub fn format_resolved(content: &str, sources: &[String]) -> String` 같은 helper 를 추가하여 frontmatter 의 sources 만 합집합으로 보정. (또는 본 task 에서 inline 처리 — 기존 `format_with_frontmatter` 로 직접 합성.)
>
> **선택**: 본 task 에서 lint.rs 변경 최소화 위해 inline 으로 `validated` 의 frontmatter 만 fix 하고 그대로 쓰기. 단순.

### 3. `build_wiki_backend` / `build_prompt_from_sessions` / `Database::sessions_by_ids`

기존 `wiki.rs` 의 backend 분기 / prompt 빌드 / DB 조회 로직 재사용:
- `build_wiki_backend`: `wiki.rs` 의 기존 dispatcher (`build_reviewer` 와 유사) 또는 inline match. P43 task 03 의 패턴 동일.
- `build_prompt_from_sessions`: `wiki.rs:200-280` 부근의 prompt 빌드 함수 재사용.
- `Database::sessions_by_ids`: `secall-core/src/store/db.rs` 에 이미 있는지 확인 — 없으면 `Vec<String>` → `Vec<SessionRow>` helper 추가 필요. 본 task 의 dependency.

> **확인 필요**: `Database::sessions_by_ids` 가 없으면 task 03 에 추가하거나, 기존 `sessions_by_project` 등 활용. 구현 시작 전 grep.

### 4. 회귀 테스트 (단위)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sources_from_frontmatter_basic() {
        let content = "---\ntype: topic\nsources:\n  - sess-A\n  - sess-B\n---\n\n## body";
        assert_eq!(
            parse_sources_from_frontmatter(content),
            vec!["sess-A".to_string(), "sess-B".to_string()]
        );
    }

    #[test]
    fn parse_sources_handles_no_sources_block() {
        let content = "---\ntype: topic\nstatus: draft\n---\n\n## body";
        assert!(parse_sources_from_frontmatter(content).is_empty());
    }

    #[test]
    fn parse_sources_stops_at_next_field() {
        let content = "---\nsources:\n  - sess-A\nstatus: draft\n  - not-a-source\n---\n";
        assert_eq!(
            parse_sources_from_frontmatter(content),
            vec!["sess-A".to_string()]
        );
    }
}
```

`unmerged_files` / `extract_sources_from_conflicted` / `finish_conflict_resolution` 의 통합 테스트는 실제 git repo 가 필요 — task 04 에서 tempfile 기반 mini-vault 로 검증.

## Dependencies

- task 01 (auto pull) — pull 후 conflict 가 있을 수 있는 상태에서 본 task 가 작동.
- task 02 (merge_with_existing 본문 정리) — 본 task 의 final write 가 task 02 의 새 동작과 정합성 유지.
- crate dep: 추가 없음.

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo check -p secall
cargo clippy --all-targets

# 2. parse_sources_from_frontmatter 단위 테스트
cargo test -p secall-core --lib vault::git::tests::parse_sources_

# 3. wiki update 통합 회귀 (기존)
cargo test -p secall --test wiki_review_resolve

# 4. (수동) 가짜 conflict 시뮬레이션
# (수동) cd /tmp/test-vault
# (수동) echo "---\nsources:\n  - sess-A\n---\nold" > wiki/x.md && git add . && git commit -m "ours"
# (수동) git checkout -b incoming && echo "---\nsources:\n  - sess-B\n---\nnew" > wiki/x.md && git add . && git commit -m "theirs"
# (수동) git checkout main && git merge incoming  # conflict 발생
# (수동) secall wiki update --no-pull  # conflict 자동 resolve 시도
# (수동) git status  # wiki/x.md 가 staged + working tree clean 이어야
```

## Risks

- **`Database::sessions_by_ids` 부재** — 기존 store API 확인 필요. 없으면 본 task 에 추가 (간단한 `WHERE id IN (...)` 쿼리). 추가 시 `secall-core/src/store/session_repo.rs` 또는 `db.rs` 에 helper.
- **충돌 시 frontmatter 의 marker 오염** — git 충돌 marker (`<<<<<<<`, `=======`, `>>>>>>>`) 가 frontmatter 안에 끼어 있으면 `parse_sources_from_frontmatter` 가 잘못 파싱할 수 있음. mitigation: stage 2/3 를 직접 `git show` 로 읽으므로 marker 없이 양쪽 원본 그대로 — 안전.
- **rebase 도중 stage 2/3 의미 반전** — `git rebase` 의 `ours/theirs` 는 사용자 직관과 반대. 본 task 는 양쪽 모두 합집합이라 의미 무관 — 영향 없음.
- **재생성 LLM 호출 비용** — conflict 가 있을 때만 호출. cross-host 동기화 시점에만 — 평소엔 비용 0.
- **재생성 backend 가 `default_backend`** — 사용자가 review 결과로 다른 backend 쓰고 싶어도 conflict resolve 는 default 만. 추후 `--resolve-backend` flag 가능 — 본 plan 영역 외.
- **finish_conflict_resolution 의 GIT_EDITOR=true** — rebase 의 commit message 편집 skip. 사용자 의도 message 손실 가능. 단, auto-resolve 는 자동화 단계라 default message 면 충분.
- **wiki/*.md 외 파일에도 충돌** — `non_wiki_conflicts` 가 비어있지 않으면 abort. 사용자가 수동 resolve 해야. 명확한 에러 메시지로 안내.
- **본문 변경 (task 02) 와의 race** — task 02 의 `merge_with_existing` 새 동작은 working-tree 의 valid 파일에만 적용. 충돌 상태에선 본 task 가 자체 처리 — race 없음.
- **`format_resolved` helper 미존재 시 인라인 처리** — task 본문에서 lint.rs 변경 최소화 위해 inline 가능. 결정은 구현자가.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/lint.rs` 의 `merge_with_existing` — task 02 영역. 본 task 는 호출 안 함 (충돌 상태라 사용 불가).
- `crates/secall/src/main.rs` — task 01 영역.
- `crates/secall/src/commands/sync.rs` — pull 패턴 reference. 변경 X.
- `crates/secall/src/commands/wiki.rs` 의 `run_review` / `build_reviewer` / `resolve_review_backend` — P43 영역.
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio,review}.rs` — backend 영역.
- `crates/secall-core/src/store/db.rs` — `sessions_by_ids` 추가가 필요하면 본 task 에서 진행, 그 외는 변경 X.
- README / docs — task 04 영역.
