use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, serde::Deserialize)]
pub struct ReviewResult {
    #[serde(default)]
    pub issues: Vec<ReviewIssue>,
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct ReviewIssue {
    pub severity: String,
    pub description: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewerKind {
    Anthropic,
    Claude,
    Codex,
    Haiku,
    Ollama,
    LmStudio,
}

#[async_trait]
pub trait WikiReviewer: Send + Sync {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult>;
}

pub struct AnthropicReviewer {
    pub api_key: String,
    pub model: String,
}

#[async_trait]
impl WikiReviewer for AnthropicReviewer {
    async fn review(&self, page_content: &str, source_summary: &str) -> Result<ReviewResult> {
        let system_prompt = load_review_system_prompt(ReviewerKind::Anthropic);
        let user_prompt =
            crate::wiki::reviewers::build_user_prompt(page_content, source_summary, false);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        let model_id = match self.model.as_str() {
            "opus" => "claude-opus-4-6",
            _ => "claude-sonnet-4-6",
        };

        let payload = serde_json::json!({
            "model": model_id,
            "max_tokens": 2048,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_prompt}
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

        parse_review_response(text)
    }
}

/// Backwards-compatible wrapper for existing call sites.
pub async fn review_page(
    api_key: &str,
    model: &str,
    page_content: &str,
    source_summary: &str,
) -> Result<ReviewResult> {
    AnthropicReviewer {
        api_key: api_key.to_string(),
        model: model.to_string(),
    }
    .review(page_content, source_summary)
    .await
}

/// 검수 응답 텍스트를 ReviewResult로 파싱
pub fn parse_review_response(text: &str) -> Result<ReviewResult> {
    let json_str = extract_json_block(text);

    serde_json::from_str::<ReviewResult>(&json_str).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse review response as JSON: {}. Raw: {}",
            e,
            &text[..text.len().min(200)]
        )
    })
}

/// 텍스트에서 JSON 블록 추출
fn extract_json_block(text: &str) -> String {
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return text[start..=end].to_string();
        }
    }
    text.to_string()
}

pub fn load_review_system_prompt(kind: ReviewerKind) -> String {
    let base = load_base_prompt();
    match kind {
        ReviewerKind::Ollama | ReviewerKind::LmStudio => {
            format!("{base}\n\n{}", load_strict_json_suffix())
        }
        _ => base,
    }
}

fn load_base_prompt() -> String {
    if let Ok(path) = std::env::var("SECALL_WIKI_REVIEW_PROMPT") {
        if let Ok(content) = std::fs::read_to_string(path) {
            return content;
        }
    }

    let custom_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("secall")
        .join("prompts")
        .join("wiki-review.md");

    if custom_path.exists() {
        std::fs::read_to_string(&custom_path).unwrap_or_default()
    } else {
        include_str!("../../../../docs/prompts/wiki-review.md").to_string()
    }
}

fn load_strict_json_suffix() -> String {
    let custom_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("secall")
        .join("prompts")
        .join("wiki-review-strict-json.md");

    if custom_path.exists() {
        std::fs::read_to_string(&custom_path).unwrap_or_default()
    } else {
        include_str!("../../../../docs/prompts/wiki-review-strict-json.md").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn assert_impl<T: WikiReviewer>() {}

    #[test]
    fn anthropic_reviewer_implements_wiki_reviewer() {
        assert_impl::<AnthropicReviewer>();
    }

    #[test]
    fn review_result_defaults_to_unapproved_no_issues() {
        let result: ReviewResult = serde_json::from_str("{}").unwrap();
        assert!(!result.approved);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_parse_review_approved() {
        let text = r#"{"issues": [], "approved": true}"#;
        let result = parse_review_response(text).unwrap();
        assert!(result.approved);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_parse_review_with_issues() {
        let text = r#"{"issues": [{"severity": "warning", "description": "Missing code snippet", "suggestion": "Add the code"}], "approved": false}"#;
        let result = parse_review_response(text).unwrap();
        assert!(!result.approved);
        assert_eq!(result.issues.len(), 1);
        assert_eq!(result.issues[0].severity, "warning");
    }

    #[test]
    fn test_parse_review_json_in_codeblock() {
        let text = "Here is my review:\n```json\n{\"issues\": [], \"approved\": true}\n```\nDone.";
        let result = parse_review_response(text).unwrap();
        assert!(result.approved);
    }

    #[test]
    fn test_extract_json_block_direct() {
        let text = r#"Some text {"issues": []} end"#;
        let json = extract_json_block(text);
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn prompt_loads_for_anthropic_kind() {
        let prompt = load_review_system_prompt(ReviewerKind::Anthropic);
        assert!(!prompt.is_empty());
        assert!(prompt.contains("위키"));
    }

    #[test]
    fn prompt_for_local_backends_includes_strict_json_suffix() {
        let anthropic = load_review_system_prompt(ReviewerKind::Anthropic);
        let ollama = load_review_system_prompt(ReviewerKind::Ollama);
        let lmstudio = load_review_system_prompt(ReviewerKind::LmStudio);
        assert!(ollama.len() > anthropic.len());
        assert!(lmstudio.len() > anthropic.len());
        assert!(ollama.contains("valid JSON object"));
        assert!(lmstudio.contains("valid JSON object"));
    }

    #[test]
    fn prompt_loads_external_file_when_present() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom-review.md");
        std::fs::write(&path, "custom prompt").unwrap();
        std::env::set_var("SECALL_WIKI_REVIEW_PROMPT", &path);
        let prompt = load_review_system_prompt(ReviewerKind::Anthropic);
        std::env::remove_var("SECALL_WIKI_REVIEW_PROMPT");
        assert_eq!(prompt, "custom prompt");
    }

    #[test]
    fn prompt_falls_back_to_embedded_when_external_missing() {
        let _guard = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("SECALL_WIKI_REVIEW_PROMPT");
        let prompt = load_review_system_prompt(ReviewerKind::Anthropic);
        assert!(prompt.contains("출력 형식"));
    }
}
