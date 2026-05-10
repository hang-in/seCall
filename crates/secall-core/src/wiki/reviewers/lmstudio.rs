use anyhow::Result;
use async_trait::async_trait;

use crate::wiki::{load_review_system_prompt, ReviewResult, ReviewerKind, WikiReviewer};

pub struct LmStudioReviewer {
    pub api_url: String,
    pub model: String,
}

#[async_trait]
impl WikiReviewer for LmStudioReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        let client = reqwest::Client::new();

        for strict in [false, true] {
            let body = serde_json::json!({
                "model": self.model,
                "messages": [
                    {
                        "role": "system",
                        "content": load_review_system_prompt(ReviewerKind::LmStudio)
                    },
                    {
                        "role": "user",
                        "content": super::build_user_prompt(page_content, source_summary, strict)
                    }
                ],
                "response_format": { "type": "json_object" }
            });

            let url = format!("{}/v1/chat/completions", self.api_url.trim_end_matches('/'));
            let resp = client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("LM Studio review request failed: {}", e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                anyhow::bail!("LM Studio review API error {}: {}", status, text);
            }

            let json: serde_json::Value = resp.json().await?;
            let text = json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}");
            if let Ok(result) = super::parse_review_response(text) {
                return Ok(result);
            }
        }

        anyhow::bail!("review JSON parse failed after retry")
    }
}
