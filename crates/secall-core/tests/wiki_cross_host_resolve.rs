use std::path::Path;
use std::process::Command;

use secall_core::vault::git::VaultGit;
use tempfile::tempdir;

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git").args(args).current_dir(dir).output().unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_vault_repo(dir: &Path) {
    run_git(dir, &["init"]);
    run_git(dir, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    std::fs::create_dir_all(dir.join("wiki")).unwrap();
}

fn commit_all(dir: &Path, message: &str) {
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", message]);
}

#[test]
fn cross_host_conflict_extracts_sources_union() {
    let dir = tempdir().unwrap();
    init_vault_repo(dir.path());

    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-base\n---\n\nbase body\n",
    )
    .unwrap();
    commit_all(dir.path(), "base");

    run_git(dir.path(), &["checkout", "-b", "incoming"]);
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-B\n---\n\ntheirs body\n",
    )
    .unwrap();
    commit_all(dir.path(), "theirs");

    run_git(dir.path(), &["checkout", "main"]);
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-A\n---\n\nours body\n",
    )
    .unwrap();
    commit_all(dir.path(), "ours");
    let merge = Command::new("git")
        .args(["merge", "incoming"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!merge.status.success(), "expected merge conflict");

    let vault_git = VaultGit::new(dir.path(), "main");
    let unmerged = vault_git.unmerged_files().unwrap();
    assert!(unmerged.iter().any(|path| path == "wiki/topic.md"));

    let sources = vault_git
        .extract_sources_from_conflicted("wiki/topic.md")
        .unwrap();
    assert_eq!(sources, vec!["sess-A".to_string(), "sess-B".to_string()]);
}

#[test]
fn finish_conflict_resolution_clears_merge_state() {
    let dir = tempdir().unwrap();
    init_vault_repo(dir.path());

    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-base\n---\n\nbase body\n",
    )
    .unwrap();
    commit_all(dir.path(), "base");

    run_git(dir.path(), &["checkout", "-b", "incoming"]);
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-B\n---\n\ntheirs body\n",
    )
    .unwrap();
    commit_all(dir.path(), "theirs");

    run_git(dir.path(), &["checkout", "main"]);
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-A\n---\n\nours body\n",
    )
    .unwrap();
    commit_all(dir.path(), "ours");
    let merge = Command::new("git")
        .args(["merge", "incoming"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!merge.status.success(), "expected merge conflict");

    let vault_git = VaultGit::new(dir.path(), "main");
    std::fs::write(
        dir.path().join("wiki/topic.md"),
        "---\ntype: topic\nsources:\n  - sess-A\n  - sess-B\n---\n\nresolved body\n",
    )
    .unwrap();
    vault_git.stage_resolved("wiki/topic.md").unwrap();
    vault_git
        .finish_conflict_resolution("auto-resolve wiki conflicts")
        .unwrap();

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());

    let final_page = std::fs::read_to_string(dir.path().join("wiki/topic.md")).unwrap();
    assert!(final_page.contains("sess-A"));
    assert!(final_page.contains("sess-B"));
}
