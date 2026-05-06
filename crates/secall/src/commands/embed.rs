use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use secall_core::{
    ingest::Session,
    store::{get_default_db_path, Database},
    vault::Config,
};

enum WorkItem {
    /// Default mode — pre-filter loaded the Session, embed pending chunks.
    /// Boxed so the enum size doesn't balloon to the Session size for the
    /// `Rebuild` variant too (clippy `large_enum_variant`).
    Cached(Box<Session>),
    /// `--all` mode — only sid; the worker reloads the Session after deleting
    /// existing vectors for wholesale rebuild.
    Rebuild(String),
}

impl WorkItem {
    fn id(&self) -> &str {
        match self {
            Self::Cached(s) => &s.id,
            Self::Rebuild(sid) => sid,
        }
    }
}

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

    // Pre-filter pass — sessions whose chunks are all already embedded (or
    // whose every turn is chunker-skip) are dropped, so [i/N] progress reflects
    // actual work. Loaded Session values are reused by the embed pass to avoid
    // a second `get_session_for_embedding` round-trip.
    //
    // `--all` skips the pre-filter and only carries sids — wholesale rebuild
    // deletes vectors and reloads inside the worker.
    let work_items: Vec<WorkItem> = if all {
        candidate_ids.into_iter().map(WorkItem::Rebuild).collect()
    } else {
        let scan_start = Instant::now();
        let total_candidates = candidate_ids.len();
        eprintln!("Scanning {total_candidates} session(s) for pending chunks...");
        let mut filtered: Vec<WorkItem> = Vec::new();
        for sid in &candidate_ids {
            let session = match db.get_session_for_embedding(sid) {
                Ok(s) => s,
                Err(_) => {
                    // surface failure in the embed pass (worker reload path)
                    filtered.push(WorkItem::Rebuild(sid.clone()));
                    continue;
                }
            };
            match indexer.has_pending_chunks(&db, &session, tz) {
                Ok(true) => filtered.push(WorkItem::Cached(Box::new(session))),
                Ok(false) => {} // silent skip
                Err(_) => filtered.push(WorkItem::Rebuild(sid.clone())),
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

    if work_items.is_empty() {
        println!("All sessions already embedded.");
        return Ok(());
    }

    let total = work_items.len();
    eprintln!(
        "Embedding {} session(s) [batch_size={}, concurrency={}]...",
        total, batch_size, concurrency
    );
    let db_path: Arc<PathBuf> = Arc::new(db_path);
    let counter = Arc::new(AtomicUsize::new(0));
    let total_chunks = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    stream::iter(work_items)
        .map(|item| {
            let indexer = Arc::clone(&indexer);
            let db_path = Arc::clone(&db_path);
            let counter = Arc::clone(&counter);
            let total_chunks = Arc::clone(&total_chunks);
            async move {
                let sid = item.id().to_string();
                let short = &sid[..sid.len().min(8)];
                let db = match Database::open(db_path.as_path()) {
                    Ok(d) => d,
                    Err(e) => {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!("  [{i}/{total}] {short} — db open failed: {e}");
                        return;
                    }
                };
                let session: Session = match item {
                    WorkItem::Cached(s) => *s,
                    WorkItem::Rebuild(sid) => {
                        // --all (또는 pre-filter 로드 실패) — 기존 vector drop 후 reload
                        if all {
                            if let Err(e) = db.delete_session_vectors(&sid) {
                                let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                                eprintln!(
                                    "  [{i}/{total}] {short} — delete-before-rebuild failed: {e}"
                                );
                                return;
                            }
                        }
                        match db.get_session_for_embedding(&sid) {
                            Ok(s) => s,
                            Err(e) => {
                                let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                                eprintln!("  [{i}/{total}] {short} — load failed: {e}");
                                return;
                            }
                        }
                    }
                };
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
                            "  [{done}/{total}] {short} — {} chunks ({:.1} chunks/s, ETA ~{eta_min}m)",
                            stats.chunks_embedded,
                            rate,
                        );
                    }
                    Err(e) => {
                        let i = counter.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!("  [{i}/{total}] {short} — embedding failed: {e}");
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
