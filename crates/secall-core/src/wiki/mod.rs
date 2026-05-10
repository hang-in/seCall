pub mod claude;
pub mod codex;
pub mod haiku;
pub mod indexer;
pub mod lint;
pub mod lmstudio;
pub mod ollama;
pub mod review;
pub mod reviewers;

pub use claude::ClaudeBackend;
pub use codex::CodexBackend;
pub use haiku::HaikuBackend;
pub use indexer::WikiIndexer;
pub use lmstudio::LmStudioBackend;
pub use ollama::OllamaBackend;
pub use review::{
    load_review_system_prompt, AnthropicReviewer, ReviewIssue, ReviewResult, ReviewerKind,
    WikiReviewer,
};
pub use reviewers::{
    ClaudeReviewer, CodexReviewer, HaikuReviewer, LmStudioReviewer, OllamaReviewer,
};

/// wiki 생성 프롬프트를 LLM에 전달하고 결과를 반환하는 추상 인터페이스
#[async_trait::async_trait]
pub trait WikiBackend: Send + Sync {
    /// 프롬프트를 전달하고 LLM 응답 텍스트를 반환한다.
    async fn generate(&self, prompt: &str) -> anyhow::Result<String>;

    /// 백엔드 이름 (로그/표시용)
    fn name(&self) -> &'static str;
}
