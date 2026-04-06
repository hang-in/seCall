use anyhow::Result;
use secall_core::{
    ingest::markdown::{extract_body_text, parse_session_frontmatter},
    store::{get_default_db_path, Database, SessionRepo},
    vault::Config,
};

pub fn run(from_vault: bool) -> Result<()> {
    if !from_vault {
        anyhow::bail!("--from-vault flag is required");
    }

    let config = Config::load_or_default();
    let db = Database::open(&get_default_db_path())?;

    let sessions_dir = config.vault.path.join("raw").join("sessions");
    if !sessions_dir.exists() {
        println!("No vault sessions directory found.");
        return Ok(());
    }

    let mut indexed = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
    {
        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to read");
                errors += 1;
                continue;
            }
        };

        let fm = match parse_session_frontmatter(&content) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse frontmatter");
                errors += 1;
                continue;
            }
        };

        if fm.session_id.is_empty() {
            tracing::warn!(path = %path.display(), "frontmatter missing session_id");
            errors += 1;
            continue;
        }

        // 중복 체크
        match db.session_exists(&fm.session_id) {
            Ok(true) => {
                skipped += 1;
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "DB check failed");
                errors += 1;
                continue;
            }
        }

        let vault_path = path
            .strip_prefix(&config.vault.path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let body = extract_body_text(&content);

        match db.insert_session_from_vault(&fm, &body, &vault_path) {
            Ok(()) => indexed += 1,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "reindex failed");
                errors += 1;
            }
        }
    }

    eprintln!(
        "\nReindex: {} indexed, {} skipped (duplicate), {} errors",
        indexed, skipped, errors
    );
    Ok(())
}
