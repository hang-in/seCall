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
    // Arc allows cloning the handle into spawn_blocking closures
    session: Arc<Mutex<ort::session::Session>>,
    tokenizer: Arc<tokenizers::Tokenizer>,
    dim: usize,
}

impl OrtEmbedder {
    pub fn new(model_dir: &Path) -> Result<Self> {
        use ort::session::builder::GraphOptimizationLevel;

        let session = ort::session::Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .commit_from_file(model_dir.join("model.onnx"))?;

        let tokenizer = tokenizers::Tokenizer::from_file(model_dir.join("tokenizer.json"))
            .map_err(|e| anyhow!("tokenizer load failed: {e}"))?;

        // Probe embedding dimension with a dummy input
        let mut sess = session;
        let dim = Self::probe_dim(&mut sess, &tokenizer).unwrap_or(1024);

        Ok(Self {
            session: Arc::new(Mutex::new(sess)),
            tokenizer: Arc::new(tokenizer),
            dim,
        })
    }

    fn probe_dim(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
    ) -> Result<usize> {
        let embedding = Self::run_inference(session, tokenizer, "test")?;
        Ok(embedding.len())
    }

    fn run_inference(
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
        let session = Arc::clone(&self.session);
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
        let session = Arc::clone(&self.session);
        let tokenizer = Arc::clone(&self.tokenizer);
        let texts: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| anyhow!("ort session lock poisoned"))?;
            let mut results = Vec::with_capacity(texts.len());
            for text in &texts {
                results.push(Self::run_inference(&mut session, &tokenizer, text)?);
            }
            Ok(results)
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
