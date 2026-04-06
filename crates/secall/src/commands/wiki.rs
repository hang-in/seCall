use std::path::PathBuf;

use anyhow::Result;
use secall_core::vault::Config;

pub async fn run_update(
    model: &str,
    since: Option<&str>,
    session: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    // 1. wiki/ directory check
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");
    if !wiki_dir.exists() {
        anyhow::bail!("wiki/ directory not found. Run `secall init` first.");
    }

    // 2. Load prompt
    let prompt = if let Some(sid) = session {
        load_incremental_prompt(sid)?
    } else {
        load_batch_prompt(since)?
    };

    // 3. dry-run: print prompt and exit
    if dry_run {
        println!("{prompt}");
        return Ok(());
    }

    // 4. Check claude CLI exists
    if !command_exists("claude") {
        anyhow::bail!(
            "Claude Code CLI not found in PATH. \
             Install: https://docs.anthropic.com/claude-code"
        );
    }

    // 5. Execute Claude Code
    let model_id = match model {
        "opus" => "claude-opus-4-6",
        _ => "claude-sonnet-4-6",
    };

    use std::io::Write as _;
    use std::process::Stdio;

    let mut child = std::process::Command::new("claude")
        .args(["-p", "--model", model_id])
        .arg("--allowedTools")
        .arg("mcp__secall__recall,mcp__secall__get,mcp__secall__status,Read,Write,Edit,Glob,Grep")
        .stdin(Stdio::piped())
        .current_dir(&config.vault.path)
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    let status = child.wait()?;

    if status.success() {
        tracing::info!("wiki update complete");
    } else {
        tracing::warn!(code = ?status.code(), "Claude Code exited with non-zero code");
    }

    Ok(())
}

pub fn run_status() -> Result<()> {
    let config = Config::load_or_default();
    let wiki_dir = config.vault.path.join("wiki");

    if !wiki_dir.exists() {
        println!("Wiki not initialized. Run `secall init`.");
        return Ok(());
    }

    let mut page_count = 0;
    for entry in walkdir::WalkDir::new(&wiki_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().map(|e| e == "md").unwrap_or(false) {
            page_count += 1;
        }
    }

    println!("Wiki: {}", wiki_dir.display());
    println!("Pages: {page_count}");
    Ok(())
}

fn load_batch_prompt(since: Option<&str>) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-update.md");
    let mut prompt = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../../docs/prompts/wiki-update.md").to_string()
    };

    if let Some(since) = since {
        prompt.push_str(&format!(
            "\n\n## 추가 조건\n- `--since {since}` 이후 세션만 검색하세요.\n"
        ));
    }

    Ok(prompt)
}

fn load_incremental_prompt(session_id: &str) -> Result<String> {
    let custom_path = prompt_dir().join("wiki-incremental.md");
    let template = if custom_path.exists() {
        std::fs::read_to_string(&custom_path)?
    } else {
        include_str!("../../../../docs/prompts/wiki-incremental.md").to_string()
    };

    Ok(template
        .replace("{SECALL_SESSION_ID}", session_id)
        .replace(
            "{SECALL_AGENT}",
            &std::env::var("SECALL_AGENT").unwrap_or_default(),
        )
        .replace(
            "{SECALL_PROJECT}",
            &std::env::var("SECALL_PROJECT").unwrap_or_default(),
        )
        .replace(
            "{SECALL_DATE}",
            &std::env::var("SECALL_DATE").unwrap_or_default(),
        ))
}

fn prompt_dir() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_PROMPTS_DIR") {
        return PathBuf::from(p);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("prompts")
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
