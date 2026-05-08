use secall_core::llm::defaults::{
    GRAPH_ANTHROPIC_DEFAULT, GRAPH_GEMINI_DEFAULT, GRAPH_LMSTUDIO_DEFAULT, GRAPH_OLLAMA_DEFAULT,
    LOG_GEMINI_DEFAULT, LOG_OLLAMA_DEFAULT, WIKI_CLAUDE_DEFAULT, WIKI_CODEX_DEFAULT,
    WIKI_REVIEW_DEFAULT,
};

#[test]
fn llm_default_constants_match_expected_values() {
    assert_eq!(GRAPH_OLLAMA_DEFAULT, "gemma4:e4b");
    assert_eq!(GRAPH_LMSTUDIO_DEFAULT, "gemma-4-e4b-it");
    assert_eq!(GRAPH_GEMINI_DEFAULT, "gemini-2.5-flash");
    assert_eq!(GRAPH_ANTHROPIC_DEFAULT, "claude-haiku-4-5-20251001");
    assert_eq!(WIKI_CLAUDE_DEFAULT, "sonnet");
    assert_eq!(WIKI_CODEX_DEFAULT, "gpt-5.4");
    assert_eq!(WIKI_REVIEW_DEFAULT, "sonnet");
    assert_eq!(LOG_OLLAMA_DEFAULT, GRAPH_OLLAMA_DEFAULT);
    assert_eq!(LOG_GEMINI_DEFAULT, GRAPH_GEMINI_DEFAULT);
}
