use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use secall_core::{
    store::{get_default_db_path, Database},
    vault::Config,
};

pub async fn run(all: bool, batch_size: Option<usize>, concurrency: usize) -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let vector_indexer = secall_core::search::vector::create_vector_indexer(&config).await;
    let Some(indexer) = vector_indexer else {
        eprintln!("No embedding backend available.");
        eprintln!("  1. Download model: secall model download");
        eprintln!("  2. Check config: [embedding] section in config.toml");
        return Ok(());
    };

    let batch_size = batch_size.unwrap_or(32);
    let indexer = Arc::new(indexer.with_batch_size(batch_size));

    let tz = config.timezone();
    let candidate_ids: Vec<String> = db.list_all_session_ids()?;

    // Pre-filter pass — drop sessions whose chunks are all already embedded
    // (or whose every turn is chunker-skip, e.g. empty content + no thinking
    // + no ToolUse). Without this filter, --all-sessions iteration would show
    // misleading [i/N] progress where N is dominated by fast no-ops. The
    // filter itself is cheap: chunker is in-memory and `get_session_chunk_keys`
    // hits the indexed `(session_id)` lookup.
    //
    // `--all` mode skips the filter — wholesale rebuild applies to every
    // session unconditionally (model change, corruption recovery).
    let session_ids: Vec<String> = if all {
        candidate_ids
    } else {
        let scan_start = Instant::now();
        let total_candidates = candidate_ids.len();
        eprintln!("Scanning {total_candidates} session(s) for pending chunks...");
        let mut filtered: Vec<String> = Vec::new();
        for sid in &candidate_ids {
            let session = match db.get_session_for_embedding(sid) {
                Ok(s) => s,
                Err(_) => {
                    // include in real_work so the per-session error path runs
                    // and the failure surfaces in the embed pass logs
                    filtered.push(sid.clone());
                    continue;
                }
            };
            match indexer.has_pending_chunks(&db, &session, tz) {
                Ok(true) => filtered.push(sid.clone()),
                Ok(false) => {} // truly no-op, skip silently
                Err(_) => filtered.push(sid.clone()),
            }
        }
        eprintln!(
            "  Scan: {} session(s) need embedding, {} skipped no-op (in {:.2}s)",
            filtered.len(),
            total_candidates - filtered.len(),
            scan_start.elapsed().as_secs_f64(),
        );
        filtered
    };

    if session_ids.is_empty() {
        println!("All sessions already embedded.");
        return Ok(());
    }

    let total = session_ids.len();
    eprintln!(
        "Embedding {} session(s) [batch_size={}, concurrency={}]...",
        total, batch_size, concurrency
    );
    let db_path: Arc<PathBuf> = Arc::new(db_path);
    let counter = Arc::new(AtomicUsize::new(0));
    let total_chunks = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    stream::iter(session_ids)
        .map(|sid| {
            let indexer = Arc::clone(&indexer);
            let db_path = Arc::clone(&db_path);
            let counter = Arc::clone(&counter);
            let total_chunks = Arc::clone(&total_chunks);
            async move {
                let db = match Database::open(db_path.as_path()) {
                    Ok(d) => d,
                    Err(e) => {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!(
                            "  [{i}/{total}] {} — db open failed: {e}",
                            &sid[..sid.len().min(8)]
                        );
                        return;
                    }
                };
                let session = match db.get_session_for_embedding(&sid) {
                    Ok(s) => s,
                    Err(e) => {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!(
                            "  [{i}/{total}] {} — load failed: {e}",
                            &sid[..sid.len().min(8)]
                        );
                        return;
                    }
                };
                // --all: wholesale rebuild — drop existing vectors so that
                // index_session re-embeds every chunk (e.g. after model change).
                // Default mode skips this and lets index_session fill only
                // missing chunks (turn-incremental healing).
                if all {
                    if let Err(e) = db.delete_session_vectors(&sid) {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!(
                            "  [{i}/{total}] {} — delete-before-rebuild failed: {e}",
                            &sid[..sid.len().min(8)]
                        );
                        return;
                    }
                }
                match indexer.index_session(&db, &session, tz).await {
                    Ok(stats) => {
                        let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let chunks_done = total_chunks
                            .fetch_add(stats.chunks_embedded, Ordering::Relaxed)
                            + stats.chunks_embedded;
                        let elapsed = start.elapsed().as_secs_f64();
                        let rate = if elapsed > 0.0 {
                            chunks_done as f64 / elapsed
                        } else {
                            0.0
                        };
                        let remaining = total - done;
                        let eta_secs = if done > 0 && elapsed > 0.0 {
                            remaining as f64 / (done as f64 / elapsed)
                        } else {
                            0.0
                        };
                        let eta_min = (eta_secs / 60.0).ceil() as u64;
                        eprintln!(
                            "  [{done}/{total}] {} — {} chunks ({:.1} chunks/s, ETA ~{eta_min}m)",
                            &sid[..sid.len().min(8)],
                            stats.chunks_embedded,
                            rate,
                        );
                    }
                    Err(e) => {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!(
                            "  [{i}/{total}] {} — embedding failed: {e}",
                            &sid[..sid.len().min(8)]
                        );
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<()>()
        .await;

    // 모든 세션 완료 후 ANN 인덱스 1회 저장
    if let Err(e) = indexer.save_ann_if_present() {
        eprintln!("Warning: ANN index save failed: {e}");
    }

    let elapsed = start.elapsed();
    let mins = elapsed.as_secs() / 60;
    let secs = elapsed.as_secs() % 60;
    let total_c = total_chunks.load(Ordering::Relaxed);
    eprintln!(
        "\nDone: {} sessions, {} chunks in {}m {}s ({:.1} chunks/s)",
        total,
        total_c,
        mins,
        secs,
        total_c as f64 / elapsed.as_secs_f64().max(0.001),
    );

    Ok(())
}
