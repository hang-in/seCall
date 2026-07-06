use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use secall_core::{
    ingest::{markdown::parse_session_turns, Session},
    store::{get_default_db_path, Database},
    vault::{resolve_session_file, Config},
};

/// 세션을 임베딩용으로 로드하되, DB turns 가 비어 있으면 vault markdown 에서 복구한다.
///
/// zero-turn 세션(과거 reindex 로 메타만 있고 turns 미저장)은 DB 만으로는 0 chunks 라
/// 임베딩되지 않는다. 이 경우 vault_path 를 해석해 실제 md 를 읽고 `parse_session_turns`
/// 로 turn 을 복원한다. 복구 불가(파일 부재/중복) 시 원래의 빈 세션을 그대로 반환해
/// 상위에서 no-op skip 되게 한다.
fn load_session_with_vault_fallback(
    db: &Database,
    vault_root: &std::path::Path,
    session_id: &str,
) -> Result<Session> {
    let mut session = db.get_session_for_embedding(session_id)?;
    if !session.turns.is_empty() {
        return Ok(session);
    }

    let Some(vault_path) = db.get_session_vault_path(session_id)? else {
        return Ok(session);
    };

    let file = match resolve_session_file(vault_root, &vault_path, session_id) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(session = %session_id, error = %e, "embed fallback: vault file unresolved");
            return Ok(session);
        }
    };

    let content = std::fs::read_to_string(&file)?;
    let turns = parse_session_turns(&content)?;
    if !turns.is_empty() {
        tracing::info!(
            session = %session_id, file = %file.display(), turns = turns.len(),
            "embed: recovered turns from vault fallback"
        );
        session.turns = turns;
    }
    Ok(session)
}

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

    // 임베딩 모델 불일치 감지: 기존 벡터가 다른 모델로 생성됐으면 교차모델 유사도가
    // 무의미해 시맨틱 검색이 조용히 저하된다(차원이 같아 에러는 안 남). 재임베딩(--all)
    // 이 아니면 경고만 하고 진행한다.
    let embed_identity = config.embedding.embedding_identity();
    let stored_model = db.get_embedding_model()?;
    // get_stats() 실패를 0 으로 뭉개면 실제 벡터가 있어도 "최초 embed" 로 오판해
    // 마커를 잘못 확정할 수 있어 에러를 전파한다(리뷰 반영).
    let existing_vectors = db.get_stats()?.vector_count;
    // backend=none 은 임베딩 비활성이라 불일치 경고가 오해만 준다 → skip.
    if config.embedding.backend != "none" && !all {
        match &stored_model {
            Some(m) if *m != embed_identity => {
                eprintln!("⚠ 임베딩 모델 불일치: 기존 벡터={m}, 현재={embed_identity}");
                eprintln!(
                    "  섞이면 시맨틱 검색 정확도가 떨어집니다. `secall embed --all` 로 전체 재임베딩을 권장합니다."
                );
            }
            None if existing_vectors > 0 => {
                eprintln!(
                    "⚠ 임베딩 모델 마커가 없습니다 (v0.7.0 이전 DB 일 수 있음). 기존 벡터가 현재 모델({embed_identity})과 다르면 `secall embed --all` 을 권장합니다."
                );
            }
            _ => {}
        }
    }

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
    let vault_root: Arc<PathBuf> = Arc::new(config.vault.path.clone());
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
            let session = match load_session_with_vault_fallback(&db, &vault_root, sid) {
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
            let vault_root = Arc::clone(&vault_root);
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
                        match load_session_with_vault_fallback(&db, &vault_root, &sid) {
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

    // 임베딩 모델 마커 갱신 — 전체 재임베딩(--all)이거나 최초 embed(기존 벡터 0 +
    // 마커 없음)일 때만 현재 모델로 확정한다. (기존 벡터가 있는데 마커만 없는
    // incremental 은 마커를 두지 않아 다음 실행에서도 경고가 유지된다.)
    if all || (stored_model.is_none() && existing_vectors == 0) {
        if let Err(e) = db.set_embedding_model(&embed_identity) {
            eprintln!("Warning: 임베딩 모델 마커 저장 실패: {e}");
        }
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
