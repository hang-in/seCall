//! Regression tests for `VaultGit::auto_commit` + `VaultGit::push`.
//!
//! 배경: 기존 `auto_commit` 가 `git add raw/ wiki/ index.md log.md .gitignore`
//! 명시 패턴을 사용했는데, vault 의 신규 디렉터리(`graph/`, `log/`)나
//! 신규 top-level 파일(`SCHEMA.md`)을 stage 하지 못해 pull rebase 가 실패했다.
//! P39 Task 00 에서 `git add -A` 로 단순화. 본 파일은 그 회귀 테스트.
//!
//! P39 리뷰 권고 (rework): `push()` 도 동일 명시 패턴이라 같은 회귀를 유발했음.
//! 같은 fix 가 push() 에도 적용됐는지 별도 검증 (bare remote 사용).

use std::path::Path;
use std::process::Command;

use secall_core::vault::git::VaultGit;
use tempfile::TempDir;

/// Initialize a fresh git repo at `path` with one initial commit so HEAD exists.
/// Configures user.email / user.name locally so `git commit` works in CI without
/// global git config. Returns the TempDir to keep the path alive.
fn init_repo_with_initial_commit() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path();

    run(path, &["init"]);
    run(path, &["config", "user.email", "test@example.com"]);
    run(path, &["config", "user.name", "Test"]);
    run(path, &["config", "commit.gpgsign", "false"]);
    // Force a known branch so test does not depend on git defaults.
    run(path, &["symbolic-ref", "HEAD", "refs/heads/main"]);

    // initial seed file so HEAD exists and subsequent `git add -A` has a base.
    std::fs::write(path.join("seed.md"), "seed\n").expect("seed write");
    run(path, &["add", "seed.md"]);
    run(path, &["commit", "-m", "init"]);

    dir
}

fn run(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("failed to run git {:?}: {}", args, e));
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn porcelain(cwd: &Path) -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(cwd)
        .output()
        .expect("git status");
    assert!(output.status.success(), "git status failed");
    String::from_utf8(output.stdout).expect("utf8")
}

fn write(path: &Path, rel: &str, content: &str) {
    let abs = path.join(rel);
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(&abs, content).expect("write");
}

#[test]
fn test_auto_commit_modified_existing_file() {
    let dir = init_repo_with_initial_commit();
    let path = dir.path();
    write(path, "index.md", "# index\n");
    run(path, &["add", "index.md"]);
    run(path, &["commit", "-m", "add index"]);

    // modify
    write(path, "index.md", "# index\nmore\n");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(committed, "should report committed=true for M state");
    assert!(
        porcelain(path).trim().is_empty(),
        "status should be clean after auto_commit"
    );
}

#[test]
fn test_auto_commit_untracked_file_in_known_dir() {
    let dir = init_repo_with_initial_commit();
    let path = dir.path();

    write(path, "raw/sessions/2026-01-01/foo.md", "session\n");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(committed);
    assert!(porcelain(path).trim().is_empty());
}

#[test]
fn test_auto_commit_untracked_new_dir() {
    // 옵션 A 검증 핵심: 명시 패턴에 없던 graph/ 도 자동 포착.
    let dir = init_repo_with_initial_commit();
    let path = dir.path();

    write(path, "graph/edges.json", "{}\n");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(committed);
    let status = porcelain(path);
    assert!(
        status.trim().is_empty(),
        "graph/edges.json should be staged & committed; status: {status:?}"
    );
}

#[test]
fn test_auto_commit_modified_top_level_md() {
    // SCHEMA.md 같은 명시 패턴 외 top-level 파일도 옵션 A 로 잡혀야 함.
    let dir = init_repo_with_initial_commit();
    let path = dir.path();
    write(path, "SCHEMA.md", "# schema v1\n");
    run(path, &["add", "SCHEMA.md"]);
    run(path, &["commit", "-m", "add schema"]);

    write(path, "SCHEMA.md", "# schema v2\n");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(committed);
    assert!(porcelain(path).trim().is_empty());
}

#[test]
fn test_auto_commit_deleted_file() {
    let dir = init_repo_with_initial_commit();
    let path = dir.path();
    write(path, "foo.md", "bye\n");
    run(path, &["add", "foo.md"]);
    run(path, &["commit", "-m", "add foo"]);

    std::fs::remove_file(path.join("foo.md")).expect("rm foo.md");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(committed, "auto_commit should stage deletions via -A");
    assert!(
        porcelain(path).trim().is_empty(),
        "deletion should be committed; status: {:?}",
        porcelain(path)
    );
}

#[test]
fn test_auto_commit_no_changes_returns_false() {
    let dir = init_repo_with_initial_commit();
    let path = dir.path();

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(!committed, "clean repo should return Ok(false)");
}

#[test]
fn test_auto_commit_non_git_dir_returns_false() {
    // No `git init` — auto_commit must not panic and must return Ok(false).
    let dir = TempDir::new().expect("tempdir");
    let git = VaultGit::new(dir.path(), "main");
    let committed = git.auto_commit().expect("auto_commit");
    assert!(!committed);
}

// ─── push() 회귀 — bare remote 로 commit + push 함께 검증 ───────────────────

/// `init_repo_with_initial_commit` + `git init --bare` remote + `push -u origin <branch>`.
/// 반환된 두 TempDir 는 호출부에서 alive 유지 (drop 시 cleanup).
/// `branch` 는 `VaultGit::new` 인자와 일치해야 함.
fn init_repo_with_bare_remote(branch: &str) -> (TempDir, TempDir) {
    let remote = TempDir::new().expect("remote tempdir");
    run(remote.path(), &["init", "--bare"]);

    let work = init_repo_with_initial_commit();
    let url = remote.path().to_str().expect("remote utf8");
    run(work.path(), &["remote", "add", "origin", url]);
    run(work.path(), &["push", "-u", "origin", branch]);

    (work, remote)
}

/// bare remote 의 지정 브랜치 트리에 있는 파일 경로 목록 (개행 분리).
/// `git init --bare` 의 default HEAD 가 main 을 안 가리킬 수 있어 ref 직접 지정.
fn remote_head_files(remote: &Path, branch: &str) -> String {
    let ref_name = format!("refs/heads/{branch}");
    let out = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", &ref_name])
        .current_dir(remote)
        .output()
        .expect("git ls-tree");
    assert!(
        out.status.success(),
        "ls-tree {ref_name} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

/// substring match 가 아닌 정확한 경로 라인 매칭 — `foo.json` 이 `foo.json.bak` 에
/// false-positive 매칭되는 것 회피.
fn contains_exact_path(file_list: &str, target: &str) -> bool {
    file_list.lines().any(|l| l == target)
}

#[test]
fn test_push_stages_new_dirs_and_top_level_files() {
    // 핵심 회귀: 명시 패턴 (raw/ wiki/ index.md log.md) 외 경로도 push() 가 모두 포함.
    let branch = "main";
    let (work, remote) = init_repo_with_bare_remote(branch);
    let wp = work.path();

    write(wp, "graph/edges.json", "{}\n");
    write(wp, "SCHEMA.md", "# v1\n");
    write(wp, "log/2026-05-06.md", "log entry\n");

    let git = VaultGit::new(wp, branch);
    let result = git
        .push("regression: ensure new dirs+files captured")
        .expect("push");
    assert!(
        result.committed > 0,
        "expected committed > 0, got {}",
        result.committed
    );
    assert!(
        porcelain(wp).trim().is_empty(),
        "local should be clean after push; status: {:?}",
        porcelain(wp)
    );

    let remote_files = remote_head_files(remote.path(), branch);
    assert!(
        contains_exact_path(&remote_files, "graph/edges.json"),
        "graph/edges.json not pushed: {remote_files}"
    );
    assert!(
        contains_exact_path(&remote_files, "SCHEMA.md"),
        "SCHEMA.md not pushed: {remote_files}"
    );
    assert!(
        contains_exact_path(&remote_files, "log/2026-05-06.md"),
        "log/2026-05-06.md not pushed: {remote_files}"
    );
}

#[test]
fn test_push_stages_deletions() {
    // 삭제도 git add -A 로 잡혀야 remote 에 반영됨 (예전 명시 패턴은 신규 dir 만 누락이 아니라
    // top-level 삭제도 누락 가능했음).
    let branch = "main";
    let (work, remote) = init_repo_with_bare_remote(branch);
    let wp = work.path();

    write(wp, "wiki/topic.md", "topic\n");
    run(wp, &["add", "wiki/topic.md"]);
    run(wp, &["commit", "-m", "add topic"]);
    run(wp, &["push", "origin", branch]);

    std::fs::remove_file(wp.join("wiki/topic.md")).expect("rm topic");

    let git = VaultGit::new(wp, branch);
    let result = git.push("regression: deletion captured").expect("push");
    assert!(
        result.committed > 0,
        "deletion should produce committed > 0"
    );
    assert!(porcelain(wp).trim().is_empty(), "local should be clean");

    let remote_files = remote_head_files(remote.path(), branch);
    assert!(
        !contains_exact_path(&remote_files, "wiki/topic.md"),
        "deletion should be reflected on remote: {remote_files}"
    );
}

#[test]
fn test_push_no_changes_returns_committed_zero() {
    let branch = "main";
    let (work, _remote) = init_repo_with_bare_remote(branch);
    let git = VaultGit::new(work.path(), branch);
    let result = git.push("noop").expect("push");
    assert_eq!(result.committed, 0, "clean repo should report 0 committed");
}

#[test]
fn test_auto_commit_respects_gitignore() {
    let dir = init_repo_with_initial_commit();
    let path = dir.path();

    write(path, ".gitignore", "*.tmp\n");
    run(path, &["add", ".gitignore"]);
    run(path, &["commit", "-m", "add gitignore"]);

    // Untracked but ignored file — must NOT be committed and must NOT block clean state.
    write(path, "scratch.tmp", "junk\n");

    let git = VaultGit::new(path, "main");
    let committed = git.auto_commit().expect("auto_commit");
    // Nothing tracked-or-stageable changed (the ignored file is invisible to add -A).
    assert!(
        !committed,
        "ignored file should not trigger a commit; auto_commit returned true"
    );
    let status = porcelain(path);
    // status --porcelain hides ignored files by default → should be empty.
    assert!(
        status.trim().is_empty(),
        "ignored .tmp should leave repo clean; status: {status:?}"
    );
}
