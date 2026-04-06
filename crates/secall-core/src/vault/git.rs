use std::path::Path;
use std::process::Command;

pub struct VaultGit<'a> {
    vault_path: &'a Path,
}

impl<'a> VaultGit<'a> {
    pub fn new(vault_path: &'a Path) -> Self {
        Self { vault_path }
    }

    pub fn is_git_repo(&self) -> bool {
        self.vault_path.join(".git").exists()
    }

    /// git init + remote 설정 + .gitignore 생성
    pub fn init(&self, remote: &str) -> crate::error::Result<()> {
        if self.is_git_repo() {
            tracing::info!("vault is already a git repo");
            return Ok(());
        }

        self.run_git(&["init"])?;
        // pull()/push()가 `origin main`을 하드코딩하므로 초기화 시 브랜치를 main으로 고정.
        // `symbolic-ref`는 첫 커밋 전에도 동작하며 모든 git 버전과 호환됨.
        self.run_git(&["symbolic-ref", "HEAD", "refs/heads/main"])?;
        self.run_git(&["remote", "add", "origin", remote])?;

        // .gitignore — DB, 캐시, Obsidian 설정 제외
        let gitignore = self.vault_path.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(
                &gitignore,
                "*.db\n*.db-wal\n*.db-shm\n*.usearch\n.DS_Store\n.obsidian/\n",
            )?;
        }

        self.run_git(&["add", "."])?;
        self.run_git(&["commit", "-m", "init: seCall vault"])?;

        tracing::info!(remote, "vault git initialized");
        Ok(())
    }

    /// git pull --rebase origin main
    pub fn pull(&self) -> crate::error::Result<PullResult> {
        if !self.is_git_repo() {
            return Ok(PullResult {
                new_files: 0,
                already_up_to_date: true,
            });
        }

        let output = self.run_git(&["pull", "--rebase", "origin", "main"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let already_up_to_date = stdout.contains("Already up to date")
            || stdout.contains("Current branch main is up to date");

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

        Ok(PullResult {
            new_files,
            already_up_to_date,
        })
    }

    /// 변경된 파일을 commit + push
    pub fn push(&self, message: &str) -> crate::error::Result<PushResult> {
        if !self.is_git_repo() {
            return Ok(PushResult { committed: 0 });
        }

        let status = self.run_git(&["status", "--porcelain"])?;
        let changes = String::from_utf8_lossy(&status.stdout);
        if changes.trim().is_empty() {
            return Ok(PushResult { committed: 0 });
        }

        let committed = changes.lines().count();

        // raw/, wiki/ 외에 vault 루트 메타데이터(index.md, log.md)도 함께 stage.
        // vault.write_session()은 index.md와 log.md를 갱신하므로 누락 시 원격 상태 불일치.
        self.run_git(&["add", "raw/", "wiki/", "index.md", "log.md"])?;
        self.run_git(&["commit", "-m", message])?;
        self.run_git(&["push", "origin", "main"])?;

        tracing::info!(committed, "vault changes pushed");
        Ok(PushResult { committed })
    }

    fn run_git(&self, args: &[&str]) -> crate::error::Result<std::process::Output> {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.vault_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::SecallError::Config(format!(
                "git {} failed: {}",
                args.join(" "),
                stderr.trim()
            )));
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
