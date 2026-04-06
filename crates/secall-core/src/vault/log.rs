use std::path::Path;

use anyhow::Result;

use crate::ingest::Session;

pub fn append_log(vault_path: &Path, session: &Session, md_path: &Path) -> Result<()> {
    let log_path = vault_path.join("log.md");

    let date = session.start_time.format("%Y-%m-%d").to_string();
    let agent = session.agent.as_str();
    let project = session.project.as_deref().unwrap_or("unknown");
    let id_prefix = &session.id[..session.id.len().min(8)];
    let turns = session.turns.len();
    let total_k = (session.total_tokens.input + session.total_tokens.output) / 1000;
    let rel_path = md_path.to_string_lossy();

    let entry = format!(
        "## [{date}] ingest | {agent} {project} 세션\n- session: {id_prefix}\n- turns: {turns}, tokens: {total_k}k\n- file: {rel_path}\n\n"
    );

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    use std::io::Write;
    f.write_all(entry.as_bytes())?;
    Ok(())
}
