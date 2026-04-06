use secall_core::search::SearchResult;

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

fn format_token_count(n: u64) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

pub fn print_search_results(results: &[SearchResult], format: &OutputFormat) {
    match format {
        OutputFormat::Text => {
            for (i, r) in results.iter().enumerate() {
                println!(
                    "{}. [{}] {} — {} (score: {:.2})",
                    i + 1,
                    r.metadata.agent,
                    r.metadata.project.as_deref().unwrap_or("?"),
                    r.metadata.date,
                    r.score
                );
                println!("   Turn {}: {}", r.turn_index + 1, r.snippet);
                if let Some(path) = &r.metadata.vault_path {
                    println!("   → {}", path);
                }
                println!();
            }
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(results).unwrap_or_default()
            );
        }
    }
}

pub fn print_ingest_result(
    session: &secall_core::ingest::Session,
    vault_path: &std::path::Path,
    stats: &secall_core::search::IndexStats,
    format: &OutputFormat,
) {
    match format {
        OutputFormat::Text => {
            println!(
                "✓ Ingested session: {}",
                &session.id[..session.id.len().min(8)]
            );
            println!(
                "  Project: {}",
                session.project.as_deref().unwrap_or("unknown")
            );
            println!("  Agent:   {}", session.agent.as_str());
            println!("  Turns:   {}", session.turns.len());
            println!(
                "  Tokens:  {} in, {} out",
                format_token_count(session.total_tokens.input),
                format_token_count(session.total_tokens.output),
            );
            println!("  File:    {}", vault_path.display());
            println!("  BM25:    {} turns indexed", stats.turns_indexed);
            if stats.chunks_embedded > 0 {
                println!("  Vectors: {} chunks embedded", stats.chunks_embedded);
            }
        }
        OutputFormat::Json => {
            let event = serde_json::json!({
                "event": "ingest_complete",
                "session_id": session.id,
                "agent": session.agent.as_str(),
                "project": session.project,
                "date": session.start_time.format("%Y-%m-%d").to_string(),
                "vault_path": vault_path.to_string_lossy(),
                "turns": session.turns.len(),
                "tokens": {
                    "input": session.total_tokens.input,
                    "output": session.total_tokens.output
                },
                "index": {
                    "bm25_indexed": stats.turns_indexed > 0,
                    "vector_indexed": stats.chunks_embedded > 0,
                    "chunks_embedded": stats.chunks_embedded
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&event).unwrap_or_default()
            );
        }
    }
}
