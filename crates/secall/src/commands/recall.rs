use anyhow::{anyhow, Result};
use secall_core::{
    search::hybrid::parse_temporal_filter,
    search::query_expand::expand_query,
    search::tokenizer::create_tokenizer,
    search::{Bm25Indexer, GraphFilter, SearchEngine, SearchFilters},
    store::{get_default_db_path, Database, RelatedSession},
    vault::Config,
};

use crate::output::{print_search_results, OutputFormat};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    query: Vec<String>,
    since: Option<String>,
    project: Option<String>,
    agent: Option<String>,
    limit: usize,
    lex_only: bool,
    vec_only: bool,
    expand: bool,
    include_automated: bool,
    no_related: bool,
    topic: Option<String>,
    file: Option<String>,
    issue: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    if query.is_empty() {
        return Err(anyhow!("Query cannot be empty"));
    }

    let query_str = query.join(" ");
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;
    let query_str = if expand {
        expand_query(&query_str, Some(&db))?
    } else {
        query_str
    };

    // Build filters
    let (since_filter, until_filter) = if let Some(since_str) = since {
        if let Some(temporal) = parse_temporal_filter(&since_str) {
            (temporal.since, temporal.until)
        } else if let Ok(dt) = chrono::NaiveDate::parse_from_str(&since_str, "%Y-%m-%d") {
            (dt.and_hms_opt(0, 0, 0).map(|dt| dt.and_utc()), None)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let exclude_session_types = if include_automated {
        vec![]
    } else {
        vec!["automated".to_string()]
    };

    // graph_filter: --topic, --file, --issue 중 첫 번째 지정된 것 적용
    let graph_filter = if let Some(t) = topic {
        Some(GraphFilter::Topic(t))
    } else if let Some(f) = file {
        Some(GraphFilter::File(f))
    } else if let Some(i) = issue {
        Some(GraphFilter::Issue(i))
    } else {
        None
    };

    let filters = SearchFilters {
        project,
        agent,
        since: since_filter,
        until: until_filter,
        exclude_session_types,
        graph_filter,
        ..Default::default()
    };

    // Build search engine
    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow!("tokenizer init failed: {e}"))?;
    let vector_indexer = if !lex_only {
        secall_core::search::vector::create_vector_indexer(&config).await
    } else {
        None
    };
    let engine = SearchEngine::new(Bm25Indexer::new(tok), vector_indexer);

    let results = if vec_only {
        engine
            .search_vector(&db, &query_str, limit, &filters)
            .await?
    } else if lex_only {
        engine.search_bm25(&db, &query_str, &filters, limit)?
    } else {
        engine.search(&db, &query_str, &filters, limit).await?
    };

    if results.is_empty() {
        println!("No results found for: {}", query_str);
        return Ok(());
    }

    print_search_results(&results, format);

    // 관련 세션 그래프 탐색 (--no-related가 없고, text 포맷일 때만)
    if !no_related && matches!(format, OutputFormat::Text) {
        let seed_ids: Vec<&str> = results
            .iter()
            .map(|r| r.session_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let related = db.get_related_sessions(&seed_ids, 2, 5).unwrap_or_default();
        print_related_sessions(&related);
    }

    Ok(())
}

fn print_related_sessions(related: &[RelatedSession]) {
    if related.is_empty() {
        return;
    }
    println!("─── 관련 세션 ───────────────────────────────────────");
    for r in related {
        let hop_label = match r.hop_count {
            1 => "직접",
            2 => "2홉",
            _ => "3홉",
        };
        println!(
            "  [{}] ({}) {} — {} {}",
            hop_label,
            r.relation,
            r.project.as_deref().unwrap_or("?"),
            r.date,
            r.agent,
        );
        if let Some(summary) = &r.summary {
            let short: String = summary.chars().take(80).collect();
            println!("      {}", short);
        }
        println!(
            "      → secall get {}",
            &r.session_id[..r.session_id.len().min(8)]
        );
        println!();
    }
}
