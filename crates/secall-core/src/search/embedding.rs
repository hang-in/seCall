use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ─── Embedder trait ───────────────────────────────────────────────────────────

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    async fn is_available(&self) -> bool;
    /// Return embedding dimension (0 = unknown / lazy)
    fn dimensions(&self) -> usize;
    fn model_name(&self) -> &str;
}

// ─── OllamaEmbedder ───────────────────────────────────────────────────────────

pub struct OllamaEmbedder {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<bool>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    pub fn new(base_url: Option<&str>, model: Option<&str>) -> Self {
        OllamaEmbedder {
            client: Client::new(),
            base_url: base_url.unwrap_or("http://localhost:11434").to_string(),
            model: model.unwrap_or("bge-m3").to_string(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut batch = self.embed_batch(&[text]).await?;
        batch
            .pop()
            .ok_or_else(|| anyhow!("empty embedding response"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let req = EmbedRequest {
            model: self.model.clone(),
            input: texts.iter().map(|s| s.to_string()).collect(),
            truncate: Some(true),
        };

        let resp = self
            .client
            .post(format!("{}/api/embed", self.base_url))
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama embed failed ({status}): {body}"));
        }

        let embed_resp: EmbedResponse = resp.json().await?;
        Ok(embed_resp.embeddings)
    }

    async fn is_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .is_ok()
    }

    fn dimensions(&self) -> usize {
        1024 // bge-m3 default
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

// ─── OrtEmbedder ─────────────────────────────────────────────────────────────

/// Local ONNX-based embedder using ort + tokenizers.
/// Requires model files at `model_dir/model.onnx` and `model_dir/tokenizer.json`.
pub struct OrtEmbedder {
    /// Session pool — multiple ORT sessions for true parallel inference.
    /// Each session is independently lockable so concurrent embed_batch calls
    /// can run on different CPU cores simultaneously.
    sessions: Vec<Arc<Mutex<ort::session::Session>>>,
    next_session: std::sync::atomic::AtomicUsize,
    tokenizer: Arc<tokenizers::Tokenizer>,
    dim: usize,
}

impl OrtEmbedder {
    pub fn new(model_dir: &Path) -> Result<Self> {
        Self::with_pool_size(model_dir, 4)
    }

    pub fn with_pool_size(model_dir: &Path, pool_size: usize) -> Result<Self> {
        use ort::session::builder::GraphOptimizationLevel;

        let pool_size = pool_size.max(1);
        let tokenizer = tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("tokenizer load failed: {e}"))?;

        // Build first session and probe dimensions
        let mut first_session = ort::session::Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_dir.join("model.onnx"))?;

        let dim = Self::probe_dim(&mut first_session, &tokenizer).unwrap_or(1024);

        let mut sessions = Vec::with_capacity(pool_size);
        sessions.push(Arc::new(Mutex::new(first_session)));

        // Build remaining sessions
        for _ in 1..pool_size {
            let sess = ort::session::Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)?
                .commit_from_file(model_dir.join("model.onnx"))?;
            sessions.push(Arc::new(Mutex::new(sess)));
        }

        tracing::info!(pool_size, dim, "ORT session pool created");

        Ok(Self {
            sessions,
            next_session: std::sync::atomic::AtomicUsize::new(0),
            tokenizer: Arc::new(tokenizer),
            dim,
        })
    }

    /// Round-robin session selection from the pool.
    fn next_session(&self) -> Arc<Mutex<ort::session::Session>> {
        let idx = self
            .next_session
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.sessions.len();
        Arc::clone(&self.sessions[idx])
    }

    pub(crate) fn probe_dim(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
    ) -> Result<usize> {
        let embedding = Self::run_inference(session, tokenizer, "test")?;
        Ok(embedding.len())
    }

    /// batch 단위로 inference. padding + attention_mask 구성 후 단일 session.run() 호출.
    pub(crate) fn run_inference_batch(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>> {
        use ndarray::Array2;
        use ort::value::TensorRef;

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // 배치 토크나이즈
        let encodings = tokenizer
            .encode_batch(texts.iter().map(|t| t.as_str()).collect::<Vec<_>>(), true)
            .map_err(|e| anyhow!("batch tokenize failed: {e}"))?;

        let batch_size = texts.len();
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        if max_len == 0 {
            return Ok(vec![Vec::new(); batch_size]);
        }

        // padding: Array2::zeros으로 zero-padding 후 실제 token으로 채움
        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));

        for (i, enc) in encodings.iter().enumerate() {
            let ids = enc.get_ids();
            let mask = enc.get_attention_mask();
            for (j, (&id, &m)) in ids.iter().zip(mask.iter()).enumerate() {
                input_ids[[i, j]] = id as i64;
                attention_mask[[i, j]] = m as i64;
            }
        }

        // 단일 session.run() — shape (batch_size, max_len)
        let ids_ref = TensorRef::<i64>::from_array_view(input_ids.view())
            .map_err(|e| anyhow!("tensor ids: {e}"))?;
        let mask_ref = TensorRef::<i64>::from_array_view(attention_mask.view())
            .map_err(|e| anyhow!("tensor mask: {e}"))?;

        let outputs = session.run(ort::inputs![
            "input_ids" => ids_ref,
            "attention_mask" => mask_ref,
        ])?;

        // last_hidden_state shape: [batch_size, max_len, dim]
        let hidden_arr = outputs["last_hidden_state"].try_extract_array::<f32>()?;
        let dim = hidden_arr.shape()[2];

        // attention_mask 기반 mean pooling + L2 normalize
        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let mask_sum = attention_mask
                .row(i)
                .iter()
                .map(|&m| m as f32)
                .sum::<f32>()
                .max(1e-9);
            let mut embedding = vec![0.0f32; dim];
            for j in 0..max_len {
                let m = attention_mask[[i, j]] as f32;
                for d in 0..dim {
                    embedding[d] += hidden_arr[[i, j, d]] * m;
                }
            }
            for e in embedding.iter_mut() {
                *e /= mask_sum;
            }
            // L2 normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-9 {
                for e in embedding.iter_mut() {
                    *e /= norm;
                }
            }
            results.push(embedding);
        }

        Ok(results)
    }

    pub(crate) fn run_inference(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
        text: &str,
    ) -> Result<Vec<f32>> {
        use ndarray::Array2;
        use ort::value::TensorRef;

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("tokenize failed: {e}"))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        let ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)
            .map_err(|e| anyhow!("array reshape: {e}"))?;
        let mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask.clone())
            .map_err(|e| anyhow!("array reshape: {e}"))?;

        // Create TensorRefs (borrows from the arrays above)
        let ids_ref = TensorRef::<i64>::from_array_view(ids_arr.view())
            .map_err(|e| anyhow!("tensor ids: {e}"))?;
        let mask_ref = TensorRef::<i64>::from_array_view(mask_arr.view())
            .map_err(|e| anyhow!("tensor mask: {e}"))?;

        let outputs = session.run(ort::inputs![
            "input_ids" => ids_ref,
            "attention_mask" => mask_ref,
        ])?;

        // Extract last_hidden_state: shape [1, seq_len, dim]
        let hidden_arr = outputs["last_hidden_state"].try_extract_array::<f32>()?;
        let shape = hidden_arr.shape();
        let dim = shape[2];

        // Mean pooling weighted by attention mask
        let mask_float: Vec<f32> = mask_arr.iter().map(|&m| m as f32).collect();
        let mask_sum: f32 = mask_float.iter().sum::<f32>().max(1e-9);
        let mut embedding = vec![0.0f32; dim];

        for i in 0..seq_len {
            let m = mask_float[i];
            for d in 0..dim {
                embedding[d] += hidden_arr[[0, i, d]] * m;
            }
        }
        for e in embedding.iter_mut() {
            *e /= mask_sum;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-9 {
            for e in embedding.iter_mut() {
                *e /= norm;
            }
        }

        Ok(embedding)
    }
}

#[async_trait]
impl Embedder for OrtEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let session = self.next_session();
        let tokenizer = Arc::clone(&self.tokenizer);
        let text = text.to_string();
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| anyhow!("ort session lock poisoned"))?;
            Self::run_inference(&mut session, &tokenizer, &text)
        })
        .await
        .map_err(|e| anyhow!("spawn_blocking join error: {e}"))?
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let session = self.next_session();
        let tokenizer = Arc::clone(&self.tokenizer);
        let texts: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| anyhow!("ort session lock poisoned"))?;
            Self::run_inference_batch(&mut session, &tokenizer, &texts)
        })
        .await
        .map_err(|e| anyhow!("spawn_blocking join error: {e}"))?
    }

    async fn is_available(&self) -> bool {
        true
    }

    fn dimensions(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        "bge-m3-onnx"
    }
}

// ─── OpenVinoEmbedder ─────────────────────────────────────────────────────────

/// Local ONNX-based embedder using openvino-rs crate directly (not ORT EP).
/// Uses a dedicated worker thread since openvino types are !Send/!Sync.
/// Requires OpenVINO runtime installed on the system.
#[cfg(feature = "openvino")]
pub struct OpenVinoEmbedder {
    tx: std::sync::Mutex<std::sync::mpsc::Sender<OpenVinoWork>>,
    _worker: Option<std::thread::JoinHandle<()>>,
    tokenizer: Arc<tokenizers::Tokenizer>,
    dim: usize,
    pub device: String,
}

#[cfg(feature = "openvino")]
struct OpenVinoWork {
    items: Vec<TokenizedItem>,
    reply: tokio::sync::oneshot::Sender<Result<Vec<Vec<f32>>>>,
}

#[cfg(feature = "openvino")]
struct TokenizedItem {
    input_ids: Vec<i64>,
    attention_mask: Vec<i64>,
}

#[cfg(feature = "openvino")]
impl OpenVinoEmbedder {
    pub fn new(model_dir: &Path, device: Option<&str>, openvino_dir: Option<&str>) -> Result<Self> {
        let device = device.unwrap_or("GPU").to_string();
        let tokenizer = tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("tokenizer load failed: {e}"))?;

        let (work_tx, work_rx) = std::sync::mpsc::channel::<OpenVinoWork>();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<usize>>();

        let model_path = model_dir.join("model.onnx").to_string_lossy().to_string();
        let device_clone = device.clone();
        let ov_dir = openvino_dir.map(|s| s.to_string());

        let worker = std::thread::spawn(move || {
            // Set OpenVINO installation directory if configured
            if let Some(dir) = &ov_dir {
                #[allow(unused_unsafe)]
                unsafe {
                    std::env::set_var("INTEL_OPENVINO_DIR", dir);
                }
            }

            // Pre-load libopenvino.so by absolute path before Core::new().
            // dlopen at runtime ignores LD_LIBRARY_PATH set after process start,
            // so we must load the dependency explicitly first.
            let _preloaded_lib = ov_dir.as_ref().and_then(|dir| {
                let base = std::path::Path::new(dir);
                let lib_dir = [
                    base.join("runtime/lib/intel64"),
                    base.join("runtime/lib"),
                    base.join("lib"),
                ]
                .into_iter()
                .find(|p| p.exists())?;

                let lib_path = lib_dir.join("libopenvino.so");
                if !lib_path.exists() {
                    tracing::warn!(path = %lib_path.display(), "libopenvino.so not found");
                    return None;
                }
                match unsafe { libloading::Library::new(&lib_path) } {
                    Ok(lib) => {
                        tracing::debug!(path = %lib_path.display(), "pre-loaded libopenvino.so");
                        Some(lib)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to pre-load libopenvino.so");
                        None
                    }
                }
            });

            let mut core = match openvino::Core::new() {
                Ok(c) => c,
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("OpenVINO Core init failed: {e:?}")));
                    return;
                }
            };

            let model = match core.read_model_from_file(&model_path, "") {
                Ok(m) => m,
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("model load failed: {e:?}")));
                    return;
                }
            };

            let device_type: openvino::DeviceType =
                openvino::DeviceType::from(device_clone.as_str()).to_owned();
            let mut compiled = match core.compile_model(&model, device_type) {
                Ok(c) => c,
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("compile for {device_clone} failed: {e:?}")));
                    return;
                }
            };

            let mut request = match compiled.create_infer_request() {
                Ok(r) => r,
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("create infer request failed: {e:?}")));
                    return;
                }
            };

            // Probe dimensions with a test inference
            match ov_infer_single(&mut request, &[101, 102], &[1, 1]) {
                Ok(embedding) => {
                    let _ = init_tx.send(Ok(embedding.len()));
                }
                Err(e) => {
                    let _ = init_tx.send(Err(anyhow!("dimension probe failed: {e}")));
                    return;
                }
            }

            // Event loop: process work items
            while let Ok(work) = work_rx.recv() {
                let mut results = Vec::with_capacity(work.items.len());
                let mut err = None;
                for item in &work.items {
                    match ov_infer_single(&mut request, &item.input_ids, &item.attention_mask) {
                        Ok(embedding) => results.push(embedding),
                        Err(e) => {
                            err = Some(e);
                            break;
                        }
                    }
                }
                let reply_result = match err {
                    Some(e) => Err(e),
                    None => Ok(results),
                };
                let _ = work.reply.send(reply_result);
            }
        });

        let dim = init_rx
            .recv()
            .map_err(|_| anyhow!("OpenVINO worker thread died during init"))??;

        tracing::info!(dim, device = %device, "OpenVINO embedder ready");

        Ok(Self {
            tx: std::sync::Mutex::new(work_tx),
            _worker: Some(worker),
            tokenizer: Arc::new(tokenizer),
            dim,
            device,
        })
    }

    fn tokenize(&self, texts: &[&str]) -> Result<Vec<TokenizedItem>> {
        texts
            .iter()
            .map(|text| {
                let encoding = self
                    .tokenizer
                    .encode(*text, true)
                    .map_err(|e| anyhow!("tokenize failed: {e}"))?;
                Ok(TokenizedItem {
                    input_ids: encoding.get_ids().iter().map(|&x| x as i64).collect(),
                    attention_mask: encoding
                        .get_attention_mask()
                        .iter()
                        .map(|&x| x as i64)
                        .collect(),
                })
            })
            .collect()
    }
}

/// Single-text inference on an OpenVINO InferRequest.
/// Returns mean-pooled, L2-normalized embedding.
#[cfg(feature = "openvino")]
fn ov_infer_single(
    request: &mut openvino::InferRequest,
    input_ids: &[i64],
    attention_mask: &[i64],
) -> Result<Vec<f32>> {
    let seq_len = input_ids.len() as i64;

    let shape = openvino::Shape::new(&[1, seq_len]).map_err(|e| anyhow!("shape error: {e:?}"))?;

    let mut ids_tensor = openvino::Tensor::new(openvino::ElementType::I64, &shape)
        .map_err(|e| anyhow!("tensor error: {e:?}"))?;
    ids_tensor
        .get_data_mut::<i64>()
        .map_err(|e| anyhow!("tensor data error: {e:?}"))?
        .copy_from_slice(input_ids);

    let mut mask_tensor = openvino::Tensor::new(openvino::ElementType::I64, &shape)
        .map_err(|e| anyhow!("tensor error: {e:?}"))?;
    mask_tensor
        .get_data_mut::<i64>()
        .map_err(|e| anyhow!("tensor data error: {e:?}"))?
        .copy_from_slice(attention_mask);

    request
        .set_tensor("input_ids", &ids_tensor)
        .map_err(|e| anyhow!("set input_ids: {e:?}"))?;
    request
        .set_tensor("attention_mask", &mask_tensor)
        .map_err(|e| anyhow!("set attention_mask: {e:?}"))?;

    request
        .infer()
        .map_err(|e| anyhow!("infer failed: {e:?}"))?;

    // Output: last_hidden_state [1, seq_len, dim]
    let output = request
        .get_output_tensor_by_index(0)
        .map_err(|e| anyhow!("get output: {e:?}"))?;
    let output_data: &[f32] = output
        .get_data()
        .map_err(|e| anyhow!("output data: {e:?}"))?;

    let seq = seq_len as usize;
    let dim = output_data.len() / seq;

    // Mean pooling weighted by attention mask
    let mask_sum: f32 = attention_mask
        .iter()
        .map(|&m| m as f32)
        .sum::<f32>()
        .max(1e-9);
    let mut embedding = vec![0.0f32; dim];
    for i in 0..seq {
        let m = attention_mask[i] as f32;
        for d in 0..dim {
            embedding[d] += output_data[i * dim + d] * m;
        }
    }
    for e in embedding.iter_mut() {
        *e /= mask_sum;
    }

    // L2 normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        for e in embedding.iter_mut() {
            *e /= norm;
        }
    }

    Ok(embedding)
}

#[cfg(feature = "openvino")]
#[async_trait]
impl Embedder for OpenVinoEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let items = self.tokenize(&[text])?;
        let embedder_tx = self
            .tx
            .lock()
            .map_err(|_| anyhow!("openvino channel lock poisoned"))?
            .clone();
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let work = OpenVinoWork {
            items,
            reply: reply_tx,
        };
        embedder_tx
            .send(work)
            .map_err(|_| anyhow!("openvino worker thread stopped"))?;
        let mut results = reply_rx
            .await
            .map_err(|_| anyhow!("openvino worker dropped reply"))??;
        results.pop().ok_or_else(|| anyhow!("empty result"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let items = self.tokenize(texts)?;
        let embedder_tx = self
            .tx
            .lock()
            .map_err(|_| anyhow!("openvino channel lock poisoned"))?
            .clone();
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        let work = OpenVinoWork {
            items,
            reply: reply_tx,
        };
        embedder_tx
            .send(work)
            .map_err(|_| anyhow!("openvino worker thread stopped"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("openvino worker dropped reply"))?
    }

    async fn is_available(&self) -> bool {
        true
    }

    fn dimensions(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        "bge-m3-openvino"
    }
}

// ─── OpenAIEmbedder ──────────────────────────────────────────────────────────

pub struct OpenAIEmbedder {
    client: Client,
    api_key: String,
    model: String,
    dim: usize,
}

#[derive(Serialize)]
struct OpenAIEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedData>,
}

#[derive(Deserialize)]
struct OpenAIEmbedData {
    embedding: Vec<f32>,
}

impl OpenAIEmbedder {
    pub fn new(api_key: &str, model: Option<&str>) -> Self {
        let model = model.unwrap_or("text-embedding-3-large").to_string();
        let dim = match model.as_str() {
            "text-embedding-3-large" => 3072,
            "text-embedding-3-small" => 1536,
            _ => 3072,
        };
        OpenAIEmbedder {
            client: Client::new(),
            api_key: api_key.to_string(),
            model,
            dim,
        }
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut batch = self.embed_batch(&[text]).await?;
        batch.pop().ok_or_else(|| anyhow!("empty response"))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let req = OpenAIEmbedRequest {
            model: self.model.clone(),
            input: texts.iter().map(|s| s.to_string()).collect(),
        };

        let resp = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI embed failed ({status}): {body}"));
        }

        let embed_resp: OpenAIEmbedResponse = resp.json().await?;
        Ok(embed_resp.data.into_iter().map(|d| d.embedding).collect())
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn dimensions(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_trait_ollama() {
        fn assert_embedder<T: Embedder>() {}
        assert_embedder::<OllamaEmbedder>();
    }

    #[test]
    fn test_embedder_trait_ort() {
        fn assert_embedder<T: Embedder>() {}
        assert_embedder::<OrtEmbedder>();
    }

    #[test]
    fn test_embedder_trait_openai() {
        fn assert_embedder<T: Embedder>() {}
        assert_embedder::<OpenAIEmbedder>();
    }

    #[test]
    fn test_openai_model_names() {
        let e_large = OpenAIEmbedder::new("key", Some("text-embedding-3-large"));
        assert_eq!(e_large.dimensions(), 3072);

        let e_small = OpenAIEmbedder::new("key", Some("text-embedding-3-small"));
        assert_eq!(e_small.dimensions(), 1536);
    }

    #[test]
    fn test_openai_missing_key_not_available() {
        let e = OpenAIEmbedder::new("", None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(!rt.block_on(e.is_available()));
    }

    #[test]
    #[ignore]
    fn test_openai_embed_real() {
        // Manual: requires OPENAI_API_KEY env var
        let key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let e = OpenAIEmbedder::new(&key, None);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let embedding = rt.block_on(e.embed("hello world")).expect("embed");
        assert_eq!(embedding.len(), 3072);
    }

    #[cfg(feature = "openvino")]
    #[test]
    fn test_embedder_trait_openvino() {
        fn assert_embedder<T: Embedder>() {}
        assert_embedder::<OpenVinoEmbedder>();
    }

    #[test]
    fn test_ollama_model_name() {
        let e = OllamaEmbedder::new(None, Some("test-model"));
        assert_eq!(e.model_name(), "test-model");
        assert_eq!(e.dimensions(), 1024);
    }

    #[test]
    #[ignore]
    fn test_ort_embed_basic() {
        // Manual test: requires ~/.cache/secall/models/bge-m3-onnx/
        let model_dir = dirs::home_dir()
            .unwrap()
            .join(".cache")
            .join("secall")
            .join("models")
            .join("bge-m3-onnx");

        let embedder = OrtEmbedder::new(&model_dir).expect("OrtEmbedder::new");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let embedding = rt.block_on(embedder.embed("hello world")).expect("embed");
        assert!(!embedding.is_empty());
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "L2 norm should be ~1.0, got {norm}"
        );
    }
}
