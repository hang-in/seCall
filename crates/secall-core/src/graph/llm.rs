// P50-B: graph semantic 추출용 LLM 백엔드 추상화.
//
// 직전까지 `semantic.rs` 안에 Anthropic / Ollama / Ollama-Cloud / OpenAI-compat
// 네 가지 호출이 각각 별도 함수로 구현되어 있었다. 요청/응답/타임아웃/에러
// 처리 패턴이 거의 동일해 보일러플레이트가 누적됐고, 새 백엔드 추가 시 분기를
// 늘려야 했다. wiki/mod.rs 의 `WikiBackend` trait 패턴을 차용해 단일
// 인터페이스로 통합한다.

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;

/// graph semantic 엣지 추출용 LLM 백엔드.
///
/// `generate` 는 system prompt 와 user prompt 를 받아 백엔드별 API 호출 →
/// LLM 응답 본문 문자열을 반환한다. JSON 파싱은 호출자(semantic.rs)가 한다.
#[async_trait]
pub(crate) trait LlmBackend: Send + Sync {
    async fn generate(&self, system: &str, user: &str) -> Result<String>;
    fn name(&self) -> &'static str;
}

const REQUEST_TIMEOUT_SECS: u64 = 120;

// ─── Anthropic ─────────────────────────────────────────────────────────────

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

pub(crate) struct AnthropicGraphBackend {
    pub api_key: String,
    pub model: String,
}

#[async_trait]
impl LlmBackend for AnthropicGraphBackend {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let request_body = serde_json::json!({
            "model": self.model,
            "max_tokens": 512,
            "system": system,
            "messages": [{"role": "user", "content": user}]
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()?;
        let resp = client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {}: {}", status, text);
        }

        let api_resp: AnthropicResponse = resp.json().await?;
        let first = api_resp
            .content
            .first()
            .ok_or_else(|| anyhow::anyhow!("Anthropic API returned empty content array"))?;
        Ok(first.text.clone())
    }
}

// ─── Ollama (local) ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

pub(crate) struct OllamaGraphBackend {
    pub base_url: String,
    pub model: String,
}

#[async_trait]
impl LlmBackend for OllamaGraphBackend {
    fn name(&self) -> &'static str {
        "ollama"
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        ollama_chat(&self.base_url, &self.model, system, user, None).await
    }
}

// ─── Ollama Cloud ──────────────────────────────────────────────────────────

pub(crate) struct OllamaCloudGraphBackend {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[async_trait]
impl LlmBackend for OllamaCloudGraphBackend {
    fn name(&self) -> &'static str {
        "ollama_cloud"
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        ollama_chat(
            &self.base_url,
            &self.model,
            system,
            user,
            Some(&self.api_key),
        )
        .await
    }
}

/// Ollama API `/api/chat` 호출 — Cloud 와 local 이 endpoint 모양은 동일하고
/// `api_key` 만 다르므로 한 함수로 합친다.
async fn ollama_chat(
    base_url: &str,
    model: &str,
    system: &str,
    user: &str,
    api_key: Option<&str>,
) -> Result<String> {
    let request_body = serde_json::json!({
        "model": model,
        "stream": false,
        "options": {"temperature": 0.1},
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    let mut req = client
        .post(&url)
        .header("content-type", "application/json")
        .json(&request_body);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let label = if api_key.is_some() {
            "Ollama Cloud"
        } else {
            "Ollama"
        };
        anyhow::bail!("{} API error {}: {}", label, status, text);
    }

    let ollama_resp: OllamaResponse = resp.json().await?;
    Ok(ollama_resp.message.content)
}

// ─── OpenAI-compat (LM Studio 등) ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    content: String,
}

pub(crate) struct OpenAiCompatGraphBackend {
    pub base_url: String,
    pub model: String,
}

#[async_trait]
impl LlmBackend for OpenAiCompatGraphBackend {
    fn name(&self) -> &'static str {
        "openai_compat"
    }

    async fn generate(&self, system: &str, user: &str) -> Result<String> {
        let request_body = serde_json::json!({
            "model": self.model,
            "temperature": 0.1,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        });

        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()?;

        let resp = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI-compat API error {}: {}", status, text);
        }

        let openai_resp: OpenAIResponse = resp.json().await?;
        if openai_resp.choices.is_empty() {
            anyhow::bail!("OpenAI-compat API returned empty choices");
        }
        Ok(openai_resp.choices[0].message.content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn ollama_response_body() -> String {
        serde_json::json!({
            "message": {"content": "{\"edges\":[]}"}
        })
        .to_string()
    }

    fn openai_compat_response_body() -> String {
        serde_json::json!({
            "choices": [{"message": {"content": "{\"edges\":[]}"}}]
        })
        .to_string()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_cloud_sends_bearer_auth_and_correct_payload() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .match_header("Authorization", "Bearer test-cloud-key")
            .match_body(Matcher::Regex(r#""model":"gemma4:31b-cloud""#.to_string()))
            .with_status(200)
            .with_body(ollama_response_body())
            .create_async()
            .await;

        let backend = OllamaCloudGraphBackend {
            base_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            api_key: "test-cloud-key".to_string(),
        };
        let text = backend
            .generate("system prompt", "user content")
            .await
            .expect("ollama_cloud generate should succeed");
        assert!(text.contains("edges"), "expected edges JSON, got: {text}");
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_cloud_propagates_http_error() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/api/chat")
            .with_status(401)
            .with_body(r#"{"error":"unauthorized"}"#)
            .create_async()
            .await;

        let backend = OllamaCloudGraphBackend {
            base_url: server.url(),
            model: "gemma4:31b-cloud".to_string(),
            api_key: "bad-key".to_string(),
        };
        let err = backend
            .generate("system", "user")
            .await
            .expect_err("401 should propagate as Err");
        assert!(
            err.to_string().contains("401"),
            "expected status 401 in error, got: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_openai_compat_sends_chat_completions_payload() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex(r#""role":"system""#.to_string()),
                Matcher::Regex(r#""role":"user""#.to_string()),
            ]))
            .with_status(200)
            .with_body(openai_compat_response_body())
            .create_async()
            .await;

        let backend = OpenAiCompatGraphBackend {
            base_url: server.url(),
            model: "gpt-4o-mini".to_string(),
        };
        let text = backend
            .generate("system prompt", "user content")
            .await
            .expect("openai_compat generate should succeed");
        assert!(text.contains("edges"), "expected edges JSON, got: {text}");
        mock.assert_async().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_openai_compat_empty_choices_returns_err() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_body(r#"{"choices":[]}"#)
            .create_async()
            .await;

        let backend = OpenAiCompatGraphBackend {
            base_url: server.url(),
            model: "gpt-4o-mini".to_string(),
        };
        let err = backend
            .generate("system", "user")
            .await
            .expect_err("empty choices should return Err");
        assert!(
            err.to_string().contains("empty choices"),
            "expected 'empty choices' in error, got: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_ollama_local_uses_chat_endpoint_and_payload() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/chat")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex(r#""role":"system""#.to_string()),
                Matcher::Regex(r#""role":"user""#.to_string()),
                Matcher::Regex(r#""stream":false"#.to_string()),
            ]))
            .with_status(200)
            .with_body(ollama_response_body())
            .create_async()
            .await;

        let backend = OllamaGraphBackend {
            base_url: server.url(),
            model: "qwen3:8b".to_string(),
        };
        let text = backend
            .generate("system", "user")
            .await
            .expect("ollama local generate should succeed");
        assert!(text.contains("edges"));
        mock.assert_async().await;
    }
}
