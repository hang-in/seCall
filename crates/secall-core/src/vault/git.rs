use std::path::Path;
use std::process::Command;

pub struct VaultGit<'a> {
    vault_path: &'a Path,
    branch: String,
}

impl<'a> VaultGit<'a> {
    pub fn new(vault_path: &'a Path, branch: &str) -> Self {
        Self {
            vault_path,
            branch: branch.to_string(),
        }
    }

    pub fn is_git_repo(&self) -> bool {
        self.vault_path.join(".git").exists()
    }

    /// vault가 rebase/merge 충돌 상태인지 확인.
    /// 충돌 상태이면 에러 메시지를 반환, 정상이면 None.
    pub fn check_conflicted_state(&self) -> Option<String> {
        if !self.is_git_repo() {
            return None;
        }

        let git_dir = self.vault_path.join(".git");

        if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            return Some(
                "Vault repo is in a rebase state. Resolve it first:\n  \
                 cd <vault> && git rebase --abort   # or fix conflicts and: git rebase --continue"
                    .to_string(),
            );
        }

        if git_dir.join("MERGE_HEAD").exists() {
            return Some(
                "Vault repo has an unfinished merge. Resolve it first:\n  \
                 cd <vault> && git merge --abort   # or fix conflicts and: git commit"
                    .to_string(),
            );
        }

        // unmerged files 확인
        if let Ok(output) = Command::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .current_dir(self.vault_path)
            .output()
        {
            let unmerged = String::from_utf8_lossy(&output.stdout);
            if !unmerged.trim().is_empty() {
                return Some(format!(
                    "Vault repo has unmerged files:\n{}\n  \
                     Resolve conflicts, then run `secall sync` again.",
                    unmerged.trim()
                ));
            }
        }

        None
    }

    /// git init + remote 설정 + .gitignore 생성
    pub fn init(&self, remote: &str) -> crate::error::Result<()> {
        if self.is_git_repo() {
            tracing::info!("vault is already a git repo");
            return Ok(());
        }

        self.run_git(&["init"])?;
        // `symbolic-ref`는 첫 커밋 전에도 동작하며 모든 git 버전과 호환됨.
        self.run_git(&[
            "symbolic-ref",
            "HEAD",
            &format!("refs/heads/{}", self.branch),
        ])?;
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

        let output = self.run_git(&["pull", "--rebase", "origin", &self.branch])?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let already_up_to_date = stdout.contains("Already up to date")
            || stdout.contains(&format!("Current branch {} is up to date", self.branch));

        let new_files = if !already_up_to_date {
            self.run_git(&["diff", "--stat", "HEAD@{1}", "HEAD"])
                .ok()
                .map(|o| count_new_session_files(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or(0)
        } else {
            0
        };

        Ok(PullResult {
            new_files,
            already_up_to_date,
        })
    }

    /// 충돌(unmerged) 상태인 파일 경로 목록을 반환.
    pub fn unmerged_files(&self) -> crate::error::Result<Vec<String>> {
        if !self.is_git_repo() {
            return Ok(vec![]);
        }

        let output = self.run_git(&["diff", "--name-only", "--diff-filter=U"])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    /// 충돌 파일의 stage 2/3에서 frontmatter `sources`를 읽어 합집합 반환.
    pub fn extract_sources_from_conflicted(&self, path: &str) -> crate::error::Result<Vec<String>> {
        let mut sources = Vec::new();

        for stage in ["2", "3"] {
            let spec = format!(":{stage}:{path}");
            let output = match self.run_git(&["show", &spec]) {
                Ok(output) => output,
                Err(_) => continue,
            };
            let content = String::from_utf8_lossy(&output.stdout);
            for source in parse_sources_from_frontmatter(&content) {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }

        Ok(sources)
    }

    /// conflict 해결 후 `git add <path>` 로 stage.
    pub fn stage_resolved(&self, path: &str) -> crate::error::Result<()> {
        self.run_git(&["add", path])?;
        Ok(())
    }

    /// merge/rebase conflict 해결 절차 마무리.
    pub fn finish_conflict_resolution(&self, message: &str) -> crate::error::Result<()> {
        if !self.is_git_repo() {
            return Ok(());
        }

        let git_dir = self.vault_path.join(".git");
        if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
            let output = Command::new("git")
                .args(["rebase", "--continue"])
                .env("GIT_EDITOR", "true")
                .current_dir(self.vault_path)
                .output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(crate::SecallError::Config(format!(
                    "git rebase --continue failed: {}",
                    stderr.trim()
                )));
            }
        } else if git_dir.join("MERGE_HEAD").exists() {
            self.run_git(&["commit", "-m", message])?;
        }

        Ok(())
    }

    /// unstaged 변경이 있으면 자동 커밋. pull 전에 호출하여 rebase 충돌 방지.
    pub fn auto_commit(&self) -> crate::error::Result<bool> {
        if !self.is_git_repo() {
            return Ok(false);
        }

        let status = self.run_git(&["status", "--porcelain"])?;
        let changes = String::from_utf8_lossy(&status.stdout);
        if changes.trim().is_empty() {
            return Ok(false);
        }

        let change_count = changes.lines().count();
        tracing::info!(
            changes = change_count,
            "auto-committing unstaged vault changes before pull"
        );

        // vault 디렉터리 안의 모든 변경을 stage (신규 dir 포함, .gitignore 가 안전망).
        self.run_git(&["add", "-A"])?;
        self.run_git(&["commit", "-m", "auto: uncommitted vault changes"])?;

        Ok(true)
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

        // vault 디렉터리 안의 모든 변경을 stage (auto_commit 과 동일 패턴).
        // 신규 dir (graph/, log/) 및 파일 (SCHEMA.md) 도 누락 없이 포착. .gitignore 안전망.
        self.run_git(&["add", "-A"])?;
        self.run_git(&["commit", "-m", message])?;
        self.run_git(&["push", "origin", &self.branch])?;

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

/// git diff --stat 출력에서 raw/sessions/ 경로가 포함된 라인 수를 카운트.
pub(crate) fn count_new_session_files(diff_stat_output: &str) -> usize {
    diff_stat_output
        .lines()
        .filter(|l| l.contains("raw/sessions/"))
        .count()
}

fn parse_sources_from_frontmatter(content: &str) -> Vec<String> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return vec![];
    };
    let Some(end) = rest.find("\n---") else {
        return vec![];
    };
    let frontmatter = &rest[..end];

    let mut in_sources = false;
    let mut sources = Vec::new();
    for line in frontmatter.lines() {
        if line.starts_with("sources:") {
            in_sources = true;
            continue;
        }
        if !in_sources {
            continue;
        }
        if let Some(source) = line.strip_prefix("  - ") {
            sources.push(source.trim().to_string());
            continue;
        }
        if !line.starts_with(' ') && !line.is_empty() {
            break;
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_single_session() {
        let output = " raw/sessions/2026-04-01/abc.md | 45 ++++\n 1 file changed";
        assert_eq!(count_new_session_files(output), 1);
    }

    #[test]
    fn test_count_multiple_mixed() {
        let output = " raw/sessions/2026-04-01/abc.md | 45 ++++\n \
                       raw/sessions/2026-04-01/def.md | 12 ++\n \
                       wiki/projects/foo.md           |  8 +\n \
                       3 files changed, 65 insertions(+)";
        assert_eq!(count_new_session_files(output), 2);
    }

    #[test]
    fn test_count_no_sessions() {
        let output = " wiki/topics/rust.md | 20 ++\n index.md | 3 +\n 2 files changed";
        assert_eq!(count_new_session_files(output), 0);
    }

    #[test]
    fn test_count_empty() {
        assert_eq!(count_new_session_files(""), 0);
    }

    #[test]
    fn test_count_summary_not_counted() {
        let output = " raw/sessions/x.md | 1 +\n 1 file changed, 1 insertion(+)";
        assert_eq!(count_new_session_files(output), 1);
    }

    #[test]
    fn test_parse_sources_from_frontmatter_basic() {
        let content = "---\ntype: topic\nsources:\n  - sess-A\n  - sess-B\n---\n\n## body";
        assert_eq!(
            parse_sources_from_frontmatter(content),
            vec!["sess-A".to_string(), "sess-B".to_string()]
        );
    }

    #[test]
    fn test_parse_sources_handles_no_sources_block() {
        let content = "---\ntype: topic\nstatus: draft\n---\n\n## body";
        assert!(parse_sources_from_frontmatter(content).is_empty());
    }

    #[test]
    fn test_parse_sources_stops_at_next_field() {
        let content = "---\nsources:\n  - sess-A\nstatus: draft\n  - not-a-source\n---\n";
        assert_eq!(
            parse_sources_from_frontmatter(content),
            vec!["sess-A".to_string()]
        );
    }
}
