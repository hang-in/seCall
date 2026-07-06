use anyhow::Result;
use secall_core::{store::get_default_db_path, store::Database, vault::Config};

pub async fn run_backfill(dry_run: bool) -> Result<()> {
    let config = Config::load_or_default();
    let classification = &config.ingest.classification;

    if classification.rules.is_empty() {
        eprintln!(
            "No classification rules found in config. \
             Add [[ingest.classification.rules]] to .secall.toml"
        );
        return Ok(());
    }

    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    // 전체 세션: (id, cwd, project, agent, first_user_content)
    let sessions = db.get_all_sessions_for_classify()?;

    let total = sessions.len();
    let mut updated = 0usize;

    let compiled_rules: Vec<super::ingest::CompiledRule> = classification
        .rules
        .iter()
        .map(|rule| {
            if let Some(pattern) = &rule.pattern {
                regex::Regex::new(pattern)
                    .map(|re| super::ingest::CompiledRule::Pattern(re, rule.session_type.clone()))
                    .map_err(|e| anyhow::anyhow!("invalid regex pattern {:?}: {}", pattern, e))
            } else if let Some(project) = &rule.project {
                Ok(super::ingest::CompiledRule::Project(
                    project.clone(),
                    rule.session_type.clone(),
                ))
            } else {
                Err(anyhow::anyhow!(
                    "classification rule missing both 'pattern' and 'project' fields"
                ))
            }
        })
        .collect::<anyhow::Result<_>>()?;

    for (session_id, _cwd, project, _agent, first_content) in &sessions {
        let new_type = super::ingest::apply_classification(
            &compiled_rules,
            first_content,
            project.as_deref(),
            &classification.default,
        );

        let short_id = session_id.chars().take(8).collect::<String>();
        if dry_run {
            eprintln!("  [dry-run] {} → {}", short_id, new_type);
        } else {
            db.update_session_type(session_id, &new_type)?;
            tracing::debug!(session = %short_id, session_type = new_type, "classified");
        }
        updated += 1;
    }

    eprintln!(
        "Classify {}complete: {}/{} sessions processed",
        if dry_run { "(dry-run) " } else { "" },
        updated,
        total,
    );
    Ok(())
}
