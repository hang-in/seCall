---
type: task
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — sync auto-commit 로직 fix (hot-fix)

## Changed files

수정:
- `crates/secall-core/src/vault/git.rs:146` — `auto_commit` 본문의 `git add raw/ wiki/ index.md log.md .gitignore` 명시 패턴이 일부 파일/디렉터리 누락. **fix 방향 2 옵션 (디벨로퍼 결정)**:
  - **옵션 A (권장)**: `git add -A` 로 단순화 — vault 디렉터리 안의 모든 변경 (`SCHEMA.md`, 신규 디렉터리 `graph/`, `log/`, 기타) 자동 포착. .gitignore 가 제외할 파일은 자동 제외.
  - **옵션 B**: 기존 명시 패턴에 `SCHEMA.md graph/ log/` 추가. 향후 신규 dir 생길 때마다 또 fix 필요 → 비추천.

신규:
- `crates/secall-core/tests/vault_auto_commit.rs` — `VaultGit::auto_commit` 회귀 통합 테스트. tempdir 안 git init + 다양한 file 상태 fixture (M / ?? / D 조합 + 새 디렉터리 + 기존 명시 패턴 외 파일) → `auto_commit()` 호출 후 `git status --porcelain` 빈 결과 검증.

## Change description

### 1. fix 옵션 A (`git add -A`) 채택 시

`vault/git.rs:146` 한 줄 변경:
```text
self.run_git(&["add", "raw/", "wiki/", "index.md", "log.md", ".gitignore"])?;
```
→
```text
self.run_git(&["add", "-A"])?;
```

장점: 누락 위험 0. .gitignore 가 안전망 (build artifacts / OS 파일 자동 제외). 향후 vault 구조 변경 시 추가 fix 불필요.

위험: vault 안에 의도치 않은 임시 파일이 있으면 commit 됨. 완화: vault `.gitignore` 가 적절히 설정되어 있어야 함 (현재 vault 의 .gitignore 점검 별도 권장 — 본 task 외).

### 2. fix 옵션 B (명시 패턴 확장) 채택 시

```text
self.run_git(&["add", "raw/", "wiki/", "graph/", "log/", "index.md", "SCHEMA.md", "log.md", ".gitignore"])?;
```

`graph/`, `log/`, `SCHEMA.md` 추가. 단점: vault 구조 변경마다 재발 가능.

**디벨로퍼 권장**: 옵션 A. 단순성 + 누락 없음 우선.

### 3. 회귀 테스트 시나리오 (vault_auto_commit.rs)

공통 fixture: tempdir + `git init` + initial commit. 시나리오:
- `test_auto_commit_modified_existing_file` — `M index.md` 상태 → auto_commit 후 status clean
- `test_auto_commit_untracked_file_in_known_dir` — `?? raw/sessions/2026-01-01/foo.md` → clean
- `test_auto_commit_untracked_new_dir` — `?? graph/edges.json` (신규 dir) → clean (옵션 A 만 통과, 옵션 B 는 graph/ 추가 시)
- `test_auto_commit_modified_top_level_md` — `M SCHEMA.md` → clean
- `test_auto_commit_deleted_file` — `D foo.md` → clean (auto_commit 가 D 도 잡음 검증)
- `test_auto_commit_no_changes_returns_false` — clean 상태에서 호출 → `Ok(false)` 반환
- `test_auto_commit_non_git_dir_returns_false` — git init 없는 dir → `Ok(false)` (panic X)
- `test_auto_commit_respects_gitignore` — `.gitignore` 에 `*.tmp` + `?? scratch.tmp` → 무시되어 clean (옵션 A 만 의미 있음)

총 7~8 tests.

### 4. tracing 메시지 보존

기존 `tracing::info!(changes = N, "auto-committing unstaged vault changes before pull")` 그대로 유지. 사용자 관점 로그 호환.

### 5. 사용자 vault 적용 절차 (참고)

본 task 는 코드 작성 + 단위 테스트 까지만. 실제 적용은:
1. P39 PR 머지 후
2. 사용자 vault 에서 현재 sync 완료 + 잔존 untracked 수동 정리 (`git add . && git commit`)
3. 다음 sync 부터 자동 정상 동작
4. 별도 회귀 검증 — sync 후 vault `git status` clean

본 절차는 task 05 README 에 안내.

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음 (sync 무관, 즉시 시작 가능)

## Verification

```bash
cargo check --tests
cargo clippy --tests --all-features -- -D warnings
cargo fmt --all -- --check
cargo test -p secall-core --test vault_auto_commit
```

7~8 tests 통과 목표.

## Risks

- **옵션 A 의 의도치 않은 commit**: vault `.gitignore` 가 부실하면 임시 파일 commit. 완화: 본 task 외에서 vault `.gitignore` 점검 권장 (사용자 책임).
- **D (삭제) 상태 처리**: `git add -A` 는 삭제도 stage. 옵션 B 는 `git add path` 로 삭제 stage 안 됨 → 옵션 B 선택 시 `git add -u` 추가 필요.
- **기존 sync 동작 회귀**: `auto_commit` 호출처는 sync 의 pull 직전 한 곳 (`commands/sync.rs:124-150` 추정). 시그니처 그대로 (`Result<bool>`) → 호출처 영향 없음.
- **회귀 테스트 git 명령 의존**: tempdir 안 `git init` 등 실제 git 명령 실행 → CI 환경에 git 설치 필요 (이미 보장됨, 기존 테스트도 git 사용).
- **tracing change_count 정확성**: 기존 `change_count = changes.lines().count()` 가 `--porcelain` 출력 라인 수. 옵션 A/B 둘 다 정확.

## Scope boundary

수정 금지:
- `crates/secall-core/src/vault/` 의 다른 파일 (`config.rs`, `index.rs`, `init.rs`, `log.rs`, `mod.rs`)
- `crates/secall/src/commands/sync.rs` — 호출처, 시그니처 무변경이므로 수정 불필요
- `crates/secall-core/src/store/`, `crates/secall-core/src/mcp/`, `crates/secall-core/src/jobs/` — 무관
- `web/`, `README*`, `.github/` — Task 05 영역 또는 무관
- vault `.gitignore` 자체 — 사용자 책임 (별도 운영 작업)
- 본 phase 의 다른 task (Task 02-05) 의 측정 보고서 파일
