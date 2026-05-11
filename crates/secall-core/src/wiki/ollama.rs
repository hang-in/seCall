use async_trait::async_trait;

use super::WikiBackend;

pub struct OllamaBackend {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_key: Option<String>,
}

#[async_trait]
impl WikiBackend for OllamaBackend {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        let client = reqwest::Client::new();
        let mut req = client
            .post(format!("{}/api/generate", self.api_url))
            .json(&serde_json::json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
                "options": { "num_predict": self.max_tokens }
            }));
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Ollama request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {body}");
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("Ollama response parse error: {}", e))?;

        json["response"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Ollama response missing 'response' field"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn ollama_generate_response() -> String {
        serde_json::json!({ "response": "wiki content" }).to_string()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_includes_bearer_auth_when_api_key_set() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/generate")
            .match_header("Authorization", "Bearer cloud-key")
            .with_status(200)
            .with_body(ollama_generate_response())
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            max_tokens: 4096,
            api_key: Some("cloud-key".to_string()),
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_ok(), "generate should succeed: {:?}", result.err());
        assert_eq!(result.unwrap(), "wiki content");
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_omits_auth_header_when_api_key_none() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/generate")
            .match_header("Authorization", Matcher::Missing)
            .with_status(200)
            .with_body(ollama_generate_response())
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "local-model".to_string(),
            max_tokens: 4096,
            api_key: None,
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_ok(), "generate without api_key should succeed: {:?}", result.err());
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_backend_generate_propagates_4xx_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/api/generate")
            .with_status(401)
            .with_body(r#"{"error":"unauthorized"}"#)
            .create_async()
            .await;

        let backend = OllamaBackend {
            api_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            max_tokens: 4096,
            api_key: Some("bad-key".to_string()),
        };

        let result = backend.generate("test prompt").await;
        assert!(result.is_err(), "4xx response should return Err");
    }
}
