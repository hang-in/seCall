mod claude;
mod codex;
mod haiku;
mod lmstudio;
mod ollama;

pub use claude::ClaudeReviewer;
pub use codex::CodexReviewer;
pub use haiku::HaikuReviewer;
pub use lmstudio::LmStudioReviewer;
pub use ollama::OllamaReviewer;

pub(crate) fn build_user_prompt(
    page_content: &str,
    source_summary: &str,
    strict_json_retry: bool,
) -> String {
    let mut prompt = format!(
        "## 위키 페이지 내용\n\n{}\n\n## 원본 세션 요약\n\n{}",
        page_content, source_summary
    );
    if strict_json_retry {
        prompt.push_str(
            "\n\n재시도 지시: 반드시 valid JSON object 만 출력하세요. \
             markdown 코드 펜스, 설명 문장, 주석은 금지입니다.",
        );
    }
    prompt
}

pub(crate) fn parse_review_response(raw: &str) -> anyhow::Result<crate::wiki::ReviewResult> {
    crate::wiki::review::parse_review_response(raw)
}
