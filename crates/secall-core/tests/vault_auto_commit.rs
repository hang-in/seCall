//! Regression tests for `VaultGit::auto_commit`.
//!
//! 배경: 기존 `auto_commit` 가 `git add raw/ wiki/ index.md log.md .gitignore`
//! 명시 패턴을 사용했는데, vault 의 신규 디렉터리(`graph/`, `log/`)나
//! 신규 top-level 파일(`SCHEMA.md`)을 stage 하지 못해 pull rebase 가 실패했다.
//! P39 Task 00 에서 `git add -A` 로 단순화. 본 파일은 그 회귀 테스트.

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
