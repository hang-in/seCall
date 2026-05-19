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

/// P83: codex/claude wiki 호출이 만든 subprocess 세션을 ingest 가 식별하기 위한
/// prompt prefix marker. `wiki::{codex,claude}::generate()` 가 prompt 앞에
/// 이 marker 를 prepend 하고, `ingest::is_noise_session` 이 첫 user turn 에서
/// 검출하면 self-ingest 루프 차단을 위해 해당 세션을 skip 한다.
/// Issue #82 fix.
pub const WIKI_INVOCATION_MARKER: &str = "<!-- secall:wiki-update -->";

/// wiki 생성 프롬프트를 LLM에 전달하고 결과를 반환하는 추상 인터페이스
#[async_trait::async_trait]
pub trait WikiBackend: Send + Sync {
    /// 프롬프트를 전달하고 LLM 응답 텍스트를 반환한다.
    async fn generate(&self, prompt: &str) -> anyhow::Result<String>;

    /// 백엔드 이름 (로그/표시용)
    fn name(&self) -> &'static str;
}
