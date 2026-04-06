use anyhow::{anyhow, Result};
use secall_core::{
    store::{get_default_db_path, Database, SessionRepo},
    vault::Config,
};

pub fn run(id: String, full: bool) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    // Parse id: either "session_id" or "session_id:turn_index"
    let (session_id, turn_index) = parse_id(&id);

    // Check session exists
    if !db.session_exists(session_id)? {
        return Err(anyhow!("Session not found: {}", session_id));
    }

    let meta = db.get_session_meta(session_id)?;

    if let Some(turn_idx) = turn_index {
        // Get specific turn
        let turn = db.get_turn(session_id, turn_idx)?;
        println!("Session: {} | Turn {}", session_id, turn_idx + 1);
        println!("Role: {}", turn.role);
        println!("---");
        println!("{}", turn.content);
    } else if full {
        // Read full MD file from vault
        let config = Config::load_or_default();
        if let Some(vault_path) = &meta.vault_path {
            let abs_path = config.vault.path.join(vault_path);
            if abs_path.exists() {
                let content = std::fs::read_to_string(&abs_path)?;
                println!("{}", content);
            } else {
                println!("Vault file not found: {}", abs_path.display());
            }
        } else {
            println!("No vault path stored for session: {}", session_id);
        }
    } else {
        // Summary
        println!("Session: {}", session_id);
        println!("Agent:   {}", meta.agent);
        if let Some(m) = &meta.model {
            println!("Model:   {}", m);
        }
        if let Some(p) = &meta.project {
            println!("Project: {}", p);
        }
        println!("Date:    {}", meta.date);
        if let Some(v) = &meta.vault_path {
            println!("File:    {}", v);
        }
    }

    Ok(())
}

fn parse_id(id: &str) -> (&str, Option<u32>) {
    if let Some(colon_pos) = id.rfind(':') {
        if let Ok(n) = id[colon_pos + 1..].parse::<u32>() {
            return (&id[..colon_pos], Some(n));
        }
    }
    (id, None)
}
