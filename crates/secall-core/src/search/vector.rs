/// Vector indexer using SQLite BLOB storage + in-memory KNN search.
///
/// Note: sqlite-vec 0.1.10-alpha.3 has a C compilation issue on the current
/// macOS environment (Darwin 25.4, arm64). We use BLOB-based storage with
/// in-memory cosine similarity as a fallback. This is functionally equivalent
/// for MVP scale (< 100k chunks).
use anyhow::Result;

#[cfg(not(target_os = "windows"))]
use super::ann::AnnIndex;
use super::bm25::{IndexStats, SearchFilters, SearchResult, SessionMeta};
use super::chunker::chunk_session;
use super::embedding::{Embedder, OllamaEmbedder, OpenAIEmbedder, OrtEmbedder};
use super::model_manager::ModelManager;
use crate::ingest::Session;
use crate::store::db::Database;
use crate::store::{SessionRepo, VectorRepo};
use crate::vault::config::Config;

/// 임베딩 벡터에 NaN 또는 Inf가 포함되어 있는지 확인
fn has_invalid_values(embedding: &[f32]) -> bool {
    embedding.iter().any(|v| v.is_nan() || v.is_infinite())
}

#[derive(Debug)]
pub struct VectorRow {
    pub rowid: i64,
    pub distance: f32,
    pub session_id: String,
    pub turn_index: u32,
    pub chunk_seq: u32,
}

pub struct VectorIndexer {
    embedder: Box<dyn Embedder>,
    /// HNSW ANN 인덱스. None이면 기존 BLOB 선형 스캔으로 fallback.
    #[cfg(not(target_os = "windows"))]
    ann_index: Option<AnnIndex>,
    batch_size: usize,
}

impl VectorIndexer {
    pub fn new(embedder: Box<dyn Embedder>) -> Self {
        VectorIndexer {
            embedder,
            #[cfg(not(target_os = "windows"))]
            ann_index: None,
            batch_size: 32,
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn with_ann(mut self, ann_index: AnnIndex) -> Self {
        self.ann_index = Some(ann_index);
        self
    }

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// ANN 인덱스를 파일에 저장. 존재하지 않으면 no-op.
    pub fn save_ann_if_present(&self) -> Result<()> {
        #[cfg(not(target_os = "windows"))]
        if let Some(ref ann) = self.ann_index {
            ann.save()?;
        }
        Ok(())
    }

    pub async fn index_session(
        &self,
        db: &Database,
        session: &Session,
        tz: chrono_tz::Tz,
    ) -> Result<IndexStats> {
        let chunks = chunk_session(session, tz);

        // Ensure vector table exists
        db.init_vector_table()?;

        // Phase 1: 임베딩 계산 — 트랜잭션 밖에서 수행 (CPU 시간 동안 DB lock 없음)
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        let batch_size = self.batch_size;
        let mut embeddings: Vec<Option<Vec<f32>>> = vec![None; chunks.len()];
        let mut embed_errors = 0usize;

        for (batch_idx, text_batch) in texts.chunks(batch_size).enumerate() {
            match self.embedder.embed_batch(text_batch).await {
                Ok(batch_embeddings) => {
                    for (i, emb) in batch_embeddings.into_iter().enumerate() {
                        let idx = batch_idx * batch_size + i;
                        if has_invalid_values(&emb) {
                            tracing::warn!(
                                session_id = %session.id,
                                chunk_idx = idx,
                                "NaN/Inf in embedding, skipping chunk"
                            );
                            embed_errors += 1;
                        } else {
                            embeddings[idx] = Some(emb);
                        }
                    }
                }
                Err(e) => {
                    // 배치 실패 → 개별 재시도
                    tracing::warn!(
                        error = %e,
                        batch = batch_idx,
                        "batch embed failed, retrying individually"
                    );
                    for (i, text) in text_batch.iter().enumerate() {
                        let idx = batch_idx * batch_size + i;
                        match self.embedder.embed(text).await {
                            Ok(emb) if !has_invalid_values(&emb) => {
                                embeddings[idx] = Some(emb);
                            }
                            Ok(_) => {
                                tracing::warn!(
                                    session_id = %session.id,
                                    chunk_idx = idx,
                                    "NaN/Inf in individual embed, skipping"
                                );
                                embed_errors += 1;
                            }
                            Err(e2) => {
                                tracing::warn!(
                                    session_id = %session.id,
                                    chunk_idx = idx,
                                    error = %e2,
                                    "individual embed failed, skipping"
                                );
                                embed_errors += 1;
                            }
                        }
                    }
                }
            }
        }

        // 유효한 임베딩이 하나도 없으면 실패, 부분 성공은 허용
        let valid_count = embeddings.iter().filter(|e| e.is_some()).count();
        if valid_count == 0 && !chunks.is_empty() {
            return Err(anyhow::anyhow!(
                "session {} embedding completely failed: 0/{} chunks embedded",
                &session.id,
                chunks.len()
            ));
        }

        if embed_errors > 0 {
            tracing::warn!(
                session_id = %session.id,
                embedded = valid_count,
                skipped = embed_errors,
                total = chunks.len(),
                "partial embedding — some chunks skipped"
            );
        }

        // Phase 2: DELETE + INSERT — 세션 단위 트랜잭션으로 원자성 보장
        // INSERT 실패 시 클로저에서 Err 반환 → with_transaction이 ROLLBACK
        // 중단 시 트랜잭션 미커밋 → DELETE도 롤백 → 기존 상태 유지
        let mut chunks_embedded = 0usize;

        db.with_transaction(|| {
            let deleted = db.delete_session_vectors(&session.id)?;
            if deleted > 0 {
                tracing::info!(session_id = %session.id, deleted, "cleaned up partial vectors");
            }

            for (chunk, emb_opt) in chunks.iter().zip(embeddings.iter()) {
                if let Some(embedding) = emb_opt {
                    let _rowid = db.insert_vector(
                        embedding,
                        &chunk.session_id,
                        chunk.turn_index,
                        chunk.seq,
                        self.embedder.model_name(),
                    )?; // Err → 클로저 종료 → ROLLBACK
                    chunks_embedded += 1;
                    #[cfg(not(target_os = "windows"))]
                    if let Some(ref ann) = self.ann_index {
                        if let Err(e) = ann.add(_rowid as u64, embedding) {
                            tracing::warn!(error = %e, "ANN index add failed");
                        }
                    }
                }
            }
            Ok(())
        })?;

        Ok(IndexStats {
            chunks_embedded,
            ..Default::default()
        })
    }

    pub async fn search(
        &self,
        db: &Database,
        query: &str,
        limit: usize,
        filters: &SearchFilters,
        candidate_session_ids: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;
        // ANN-aware 경로를 공통으로 사용
        self.search_with_embedding(db, &query_embedding, limit, filters, candidate_session_ids)
    }

    /// Embed a query string without DB access (safe to call before locking DB mutex).
    pub async fn embed_query(&self, query: &str) -> anyhow::Result<Vec<f32>> {
        self.embedder.embed(query).await
    }

    /// Search vectors using a pre-computed embedding (sync, no async needed).
    pub fn search_with_embedding(
        &self,
        db: &Database,
        embedding: &[f32],
        limit: usize,
        filters: &SearchFilters,
        candidate_session_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        // ANN 경로: session_ids 필터 없고 ANN 인덱스 사용 가능할 때
        #[cfg(not(target_os = "windows"))]
        if candidate_session_ids.is_none() {
            if let Some(ref ann) = self.ann_index {
                // Stale guard (크기 기반): ANN이 DB보다 작으면 새 벡터가 ANN에 없음 → BLOB 스캔
                let db_count = db.count_vectors().unwrap_or(0);
                if ann.size() < db_count {
                    tracing::info!(
                        ann_size = ann.size(),
                        db_count,
                        "ANN index stale (size < db_count), falling back to BLOB scan"
                    );
                    // fall through to BLOB scan
                } else {
                    // Stale guard (rowid 기반): ANN은 add-only라 re-embed/--all 후
                    // 삭제된 옛 rowid가 남아 size >= db_count를 통과할 수 있음.
                    // get_vector_meta 실패(DB에 없는 rowid)가 하나라도 나오면 stale로 판단.
                    let ann_results = ann.search(embedding, limit)?;
                    let mut stale_found = false;
                    let mut results = Vec::with_capacity(ann_results.len());

                    for (key, distance) in &ann_results {
                        match db.get_vector_meta(*key as i64) {
                            Ok((session_id, turn_index, _chunk_seq)) => {
                                if let Ok(meta) = db.get_session_meta(&session_id) {
                                    if passes_filters(&meta, filters) {
                                        results.push(SearchResult {
                                            session_id,
                                            turn_index,
                                            score: 1.0 - *distance as f64,
                                            bm25_score: None,
                                            vector_score: Some(1.0 - *distance as f64),
                                            snippet: String::new(),
                                            metadata: meta,
                                        });
                                    }
                                }
                            }
                            Err(_) => {
                                // rowid가 DB에 없음: re-embed/--all 후 DELETE된 row의 잔재
                                stale_found = true;
                            }
                        }
                    }

                    if stale_found {
                        tracing::info!(
                            ann_size = ann.size(),
                            db_count,
                            "stale ANN entries detected (post-reembed rowids), falling back to BLOB scan"
                        );
                        // fall through to BLOB scan
                    } else {
                        return Ok(results);
                    }
                }
            }
        }

        // BLOB 선형 스캔 fallback
        let rows = db.search_vectors(embedding, limit, candidate_session_ids)?;
        let results = rows
            .into_iter()
            .filter_map(|row| {
                let meta = db.get_session_meta(&row.session_id).ok()?;
                if !passes_filters(&meta, filters) {
                    return None;
                }
                Some(SearchResult {
                    session_id: row.session_id,
                    turn_index: row.turn_index,
                    score: 1.0 - row.distance as f64,
                    bm25_score: None,
                    vector_score: Some(1.0 - row.distance as f64),
                    snippet: String::new(),
                    metadata: meta,
                })
            })
            .collect();
        Ok(results)
    }
}

/// Check whether a session's metadata satisfies project/agent/date filters.
pub fn passes_filters(meta: &SessionMeta, filters: &SearchFilters) -> bool {
    if let Some(proj) = &filters.project {
        if meta.project.as_deref() != Some(proj.as_str()) {
            return false;
        }
    }
    if let Some(ag) = &filters.agent {
        if meta.agent != *ag {
            return false;
        }
    }
    // Date comparison against "YYYY-MM-DD" in meta.date
    if filters.since.is_some() || filters.until.is_some() {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&meta.date, "%Y-%m-%d") {
            if let Some(since) = filters.since {
                if date < since.date_naive() {
                    return false;
                }
            }
            if let Some(until) = filters.until {
                if date >= until.date_naive() {
                    return false;
                }
            }
        }
    }
    if !filters.exclude_session_types.is_empty()
        && filters.exclude_session_types.contains(&meta.session_type)
    {
        return false;
    }
    true
}

/// Create a VectorIndexer based on config.embedding.backend.
/// Falls back to Ollama if ort fails; returns None if neither is available.
pub async fn create_vector_indexer(config: &Config) -> Option<VectorIndexer> {
    let indexer = match config.embedding.backend.as_str() {
        "ort" => {
            let model_dir = config
                .embedding
                .model_path
                .clone()
                .unwrap_or_else(default_model_path);

            // Auto-download model if not fully present (model.onnx + tokenizer.json)
            let mgr = ModelManager::new(model_dir.clone());
            if !mgr.is_downloaded() {
                tracing::warn!("ONNX model not found, downloading");
                if let Err(e) = mgr.download(false).await {
                    tracing::warn!(error = %e, "download failed, trying Ollama fallback");
                    return try_ollama_fallback_with_ann(config).await;
                }
            }

            match OrtEmbedder::new(&model_dir) {
                Ok(e) => {
                    tracing::info!("ort ONNX loaded, local vector search enabled");
                    VectorIndexer::new(Box::new(e))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "ort load failed, trying Ollama fallback");
                    return try_ollama_fallback_with_ann(config).await;
                }
            }
        }
        #[cfg(feature = "openvino")]
        "openvino" => {
            let model_dir = config
                .embedding
                .model_path
                .clone()
                .unwrap_or_else(default_model_path);

            let mgr = ModelManager::new(model_dir.clone());
            if !mgr.is_downloaded() {
                tracing::warn!("ONNX model not found, downloading");
                if let Err(e) = mgr.download(false).await {
                    tracing::warn!(error = %e, "download failed, trying ORT CPU fallback");
                    return try_ort_cpu_fallback(config).await;
                }
            }

            let device = config.embedding.openvino_device.as_deref();
            let ov_dir = config.openvino.dir.as_deref();
            match crate::search::embedding::OpenVinoEmbedder::new(&model_dir, device, ov_dir) {
                Ok(e) => {
                    tracing::info!(device = %e.device, "OpenVINO loaded, NPU vector search enabled");
                    VectorIndexer::new(Box::new(e))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "OpenVINO load failed, trying ORT CPU fallback");
                    return try_ort_cpu_fallback(config).await;
                }
            }
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
            if !api_key.is_empty() {
                let model = config.embedding.openai_model.as_deref();
                let embedder = OpenAIEmbedder::new(&api_key, model);
                tracing::info!(model = %embedder.model_name(), "OpenAI embedder ready");
                VectorIndexer::new(Box::new(embedder))
            } else {
                tracing::warn!("OPENAI_API_KEY not set, trying Ollama fallback");
                return try_ollama_fallback_with_ann(config).await;
            }
        }
        _ => {
            // "ollama" or any unknown value → Ollama
            return try_ollama_fallback_with_ann(config).await;
        }
    };

    #[cfg(not(target_os = "windows"))]
    let indexer = attach_ann_index(indexer);
    Some(indexer)
}

/// OpenVINO 실패 시 ORT CPU → Ollama 순으로 fallback.
#[cfg(feature = "openvino")]
async fn try_ort_cpu_fallback(config: &Config) -> Option<VectorIndexer> {
    let model_dir = config
        .embedding
        .model_path
        .clone()
        .unwrap_or_else(default_model_path);

    match OrtEmbedder::new(&model_dir) {
        Ok(e) => {
            tracing::info!("ORT CPU fallback loaded, vector search enabled");
            let indexer = VectorIndexer::new(Box::new(e));
            #[cfg(not(target_os = "windows"))]
            let indexer = attach_ann_index(indexer);
            Some(indexer)
        }
        Err(e) => {
            tracing::warn!(error = %e, "ORT CPU fallback also failed, trying Ollama");
            try_ollama_fallback_with_ann(config).await
        }
    }
}

async fn try_ollama_fallback_with_ann(config: &Config) -> Option<VectorIndexer> {
    let base_url = config.embedding.ollama_url.as_deref();
    let model = config.embedding.ollama_model.as_deref();
    let embedder = OllamaEmbedder::new(base_url, model);
    if embedder.is_available().await {
        tracing::info!("Ollama available, vector search enabled");
        let indexer = VectorIndexer::new(Box::new(embedder));
        #[cfg(not(target_os = "windows"))]
        let indexer = attach_ann_index(indexer);
        Some(indexer)
    } else {
        tracing::warn!("Ollama not available, vector search disabled, BM25-only mode");
        None
    }
}

#[cfg(not(target_os = "windows"))]
/// ANN 인덱스 파일을 로드(또는 생성)하여 VectorIndexer에 붙임.
/// 로드 실패 시 ANN 없이 반환 (graceful degradation).
fn attach_ann_index(indexer: VectorIndexer) -> VectorIndexer {
    let dimensions = indexer.embedder.dimensions();
    if dimensions == 0 {
        // 차원을 알 수 없으면 ANN 인덱스 생성 불가
        return indexer;
    }

    let model_name = indexer.embedder.model_name().replace(['/', ':'], "_");
    let file_name = format!("ann_{}_{}.usearch", model_name, dimensions);
    let ann_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("secall")
        .join(file_name);

    match AnnIndex::open_or_create(&ann_path, dimensions) {
        Ok(ann) => {
            tracing::info!(
                dimensions,
                path = %ann_path.display(),
                "ANN index attached to VectorIndexer"
            );
            indexer.with_ann(ann)
        }
        Err(e) => {
            tracing::warn!(error = %e, "ANN index unavailable, falling back to BLOB scan");
            indexer
        }
    }
}

fn default_model_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".cache")
        .join("secall")
        .join("models")
        .join("bge-m3-onnx")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Database;
    use crate::store::vector_repo::{bytes_to_floats, cosine_distance};

    #[test]
    fn test_vector_indexer_with_trait_object() {
        // Compile-time check: Box<dyn Embedder> works as VectorIndexer embedder
        let embedder: Box<dyn Embedder> = Box::new(OllamaEmbedder::new(None, None));
        let _indexer = VectorIndexer::new(embedder);
    }

    #[test]
    fn test_init_vector_table() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        // Re-init should be idempotent
        db.init_vector_table().unwrap();
    }

    #[test]
    fn test_insert_and_search_vectors() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();

        let emb1: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb2: Vec<f32> = vec![0.0, 1.0, 0.0];
        let query: Vec<f32> = vec![1.0, 0.1, 0.0];

        db.insert_vector(&emb1, "s1", 0, 0, "bge-m3").unwrap();
        db.insert_vector(&emb2, "s2", 0, 0, "bge-m3").unwrap();

        let rows = db.search_vectors(&query, 2, None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].session_id, "s1");
    }

    #[test]
    fn test_search_vectors_with_session_filter() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();

        db.insert_vector(&[1.0_f32, 0.0, 0.0], "s1", 0, 0, "test")
            .unwrap();
        db.insert_vector(&[0.0_f32, 1.0, 0.0], "s2", 0, 0, "test")
            .unwrap();

        let query = vec![1.0_f32, 0.1, 0.0];
        let rows = db
            .search_vectors(&query, 10, Some(&["s1".to_string()]))
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, "s1");
    }

    #[test]
    fn test_search_vectors_empty_filter_returns_empty() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();

        db.insert_vector(&[1.0_f32, 0.0, 0.0], "s1", 0, 0, "test")
            .unwrap();

        let query = vec![1.0_f32, 0.0, 0.0];
        let rows = db.search_vectors(&query, 10, Some(&[])).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_insert_vector_empty_rejected() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        let result = db.insert_vector(&[], "s1", 0, 0, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_insert_vector_dimension_mismatch() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();

        db.insert_vector(&[1.0_f32, 0.0, 0.0], "s1", 0, 0, "test")
            .unwrap();

        let result = db.insert_vector(&[1.0_f32, 0.0], "s2", 0, 0, "test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("dimension mismatch"));
    }

    #[test]
    fn test_bytes_to_floats_corrupt_blob() {
        let result = bytes_to_floats(&[0, 0, 0, 0, 0]); // 5 bytes
        assert!(result.is_empty());
    }

    #[test]
    fn test_has_invalid_values() {
        assert!(!has_invalid_values(&[1.0, 2.0, 3.0]));
        assert!(has_invalid_values(&[1.0, f32::NAN, 3.0]));
        assert!(has_invalid_values(&[1.0, f32::INFINITY, 3.0]));
        assert!(has_invalid_values(&[f32::NEG_INFINITY]));
        assert!(!has_invalid_values(&[]));
    }

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!((cosine_distance(&a, &b) - 0.0).abs() < 0.001);

        let c = vec![0.0, 1.0];
        assert!((cosine_distance(&a, &c) - 1.0).abs() < 0.001);
    }
}
