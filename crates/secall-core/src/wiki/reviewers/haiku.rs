use anyhow::Result;
use async_trait::async_trait;

use crate::wiki::{load_review_system_prompt, ReviewResult, ReviewerKind, WikiReviewer};

pub struct HaikuReviewer {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
}

#[async_trait]
impl WikiReviewer for HaikuReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        for strict in [false, true] {
            let payload = serde_json::json!({
                "model": self.model,
                "max_tokens": self.max_tokens,
                "system": load_review_system_prompt(ReviewerKind::Haiku),
                "messages": [
                    {
                        "role": "user",
                        "content": super::build_user_prompt(page_content, source_summary, strict)
                    }
                ]
            });

            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Review API request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Review API error {}: {}", status, body);
            }

            let json: serde_json::Value = resp.json().await?;
            let text = json["content"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|block| block["text"].as_str())
                .unwrap_or("{}");

            if let Ok(result) = super::parse_review_response(text) {
                return Ok(result);
            }
        }

        anyhow::bail!("review JSON parse failed after retry")
    }
}
