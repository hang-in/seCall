use secall_core::llm::defaults::{
    GRAPH_ANTHROPIC_DEFAULT, GRAPH_LMSTUDIO_DEFAULT, GRAPH_OLLAMA_CLOUD_DEFAULT,
    GRAPH_OLLAMA_DEFAULT, LOG_CONTEXT_CHAR_LIMIT, LOG_OLLAMA_CLOUD_DEFAULT, LOG_OLLAMA_DEFAULT,
    WIKI_CLAUDE_DEFAULT, WIKI_CODEX_DEFAULT, WIKI_REVIEW_DEFAULT,
};

#[test]
fn llm_default_constants_match_expected_values() {
    assert_eq!(GRAPH_OLLAMA_DEFAULT, "gemma4:e4b");
    assert_eq!(GRAPH_LMSTUDIO_DEFAULT, "gemma-4-e4b-it");
    assert_eq!(GRAPH_ANTHROPIC_DEFAULT, "claude-haiku-4-5-20251001");
    assert_eq!(GRAPH_OLLAMA_CLOUD_DEFAULT, "gemma4:31b-cloud");
    assert_eq!(WIKI_CLAUDE_DEFAULT, "sonnet");
    assert_eq!(WIKI_CODEX_DEFAULT, "gpt-5.4");
    assert_eq!(WIKI_REVIEW_DEFAULT, "haiku");
    assert_eq!(LOG_OLLAMA_DEFAULT, GRAPH_OLLAMA_DEFAULT);
    assert_eq!(LOG_OLLAMA_CLOUD_DEFAULT, "kimi-k2.6:cloud");
}

// LOG_CONTEXT_CHAR_LIMIT 는 const 라 compile-time 검증이 더 적합.
// clippy(assertions_on_constants) 회피.
const _: () = {
    assert!(
        LOG_CONTEXT_CHAR_LIMIT >= 200_000 && LOG_CONTEXT_CHAR_LIMIT <= 1_000_000,
        "LOG_CONTEXT_CHAR_LIMIT is outside expected range"
    );
};
