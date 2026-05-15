use async_trait::async_trait;

use super::WikiBackend;

pub struct LmStudioBackend {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
}

#[async_trait]
impl WikiBackend for LmStudioBackend {
    fn name(&self) -> &'static str {
        "lmstudio"
    }

    async fn generate(&self, prompt: &str) -> anyhow::Result<String> {
        // P52: LM Studio server hang 회피. wiki 생성은 출력이 길어 300s 한도.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let resp = client
            .post(format!("{}/v1/chat/completions", self.api_url))
            .json(&serde_json::json!({
                "model": self.model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": self.max_tokens,
                "stream": false
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("LM Studio request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LM Studio API error: {body}");
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("LM Studio response parse error: {}", e))?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("LM Studio response missing content field"))
    }
}
