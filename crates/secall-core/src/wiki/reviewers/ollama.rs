use anyhow::Result;
use async_trait::async_trait;

use crate::wiki::{load_review_system_prompt, ReviewResult, ReviewerKind, WikiReviewer};

pub struct OllamaReviewer {
    pub api_url: String,
    pub model: String,
    /// P55: Ollama Cloud 호출 시 bearer auth 사용. None 이면 local Ollama.
    pub api_key: Option<String>,
}

#[async_trait]
impl WikiReviewer for OllamaReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        for strict in [false, true] {
            let body = serde_json::json!({
                "model": self.model,
                "messages": [
                    {
                        "role": "system",
                        "content": load_review_system_prompt(ReviewerKind::Ollama)
                    },
                    {
                        "role": "user",
                        "content": super::build_user_prompt(page_content, source_summary, strict)
                    }
                ],
                "stream": false,
                "format": "json"
            });

            let url = format!("{}/api/chat", self.api_url.trim_end_matches('/'));
            let mut req = client.post(&url).json(&body);
            if let Some(key) = &self.api_key {
                req = req.bearer_auth(key);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Ollama review request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("Ollama review API error {}: {}", status, text);
            }

            let json: serde_json::Value = resp.json().await?;
            let text = json["message"]["content"].as_str().unwrap_or("{}");
            if let Ok(result) = super::parse_review_response(text) {
                return Ok(result);
            }
        }

        anyhow::bail!("review JSON parse failed after retry")
    }
}
