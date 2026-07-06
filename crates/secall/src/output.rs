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

/// 세션 id 표시용 8글자 prefix.
///
/// byte-slice (`&id[..8]`) 는 멀티바이트 세션 id 에서
/// "byte index 8 is not a char boundary" panic 을 내므로 char 단위로 자른다.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
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
            println!("✓ Ingested session: {}", short_id(&session.id));
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
            // JSON 모드에서는 세션별 이벤트를 출력하지 않음.
            // run()에서 단일 summary JSON을 출력하여 top-level JSON 문서가 하나만 나오도록 함.
            let _ = (session, vault_path, stats);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::short_id;

    #[test]
    fn short_id_ascii() {
        assert_eq!(short_id("0123456789abcdef"), "01234567");
        assert_eq!(short_id("abc"), "abc");
    }

    #[test]
    fn short_id_multibyte_no_panic() {
        // 멀티바이트 세션 id 를 byte-slice (`&id[..8]`) 로 자르면
        // "byte index 8 is not a char boundary" 로 panic 한다.
        // char 단위로 자르면 안전하게 앞 8글자를 반환해야 한다.
        let id = "한글세션식별자입니다";
        let short = short_id(id);
        assert_eq!(short.chars().count(), 8);
        assert_eq!(short, "한글세션식별자입");
    }
}
