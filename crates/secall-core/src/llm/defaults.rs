use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

pub const GRAPH_OLLAMA_DEFAULT: &str = "gemma4:e4b";
pub const GRAPH_LMSTUDIO_DEFAULT: &str = "gemma-4-e4b-it";
pub const GRAPH_GEMINI_DEFAULT: &str = "gemini-2.5-flash";
pub const GRAPH_ANTHROPIC_DEFAULT: &str = "claude-haiku-4-5-20251001";
pub const WIKI_CLAUDE_DEFAULT: &str = "sonnet";
pub const WIKI_CODEX_DEFAULT: &str = "gpt-5.4";
pub const WIKI_REVIEW_DEFAULT: &str = "sonnet";
pub const LOG_OLLAMA_DEFAULT: &str = GRAPH_OLLAMA_DEFAULT;
pub const LOG_GEMINI_DEFAULT: &str = GRAPH_GEMINI_DEFAULT;

fn warned_fields() -> &'static Mutex<HashSet<&'static str>> {
    static WARNED_FIELDS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    WARNED_FIELDS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn warn_using_default(field: &'static str, value: &'static str) {
    let mut warned = warned_fields()
        .lock()
        .expect("llm default warning mutex poisoned");
    if warned.insert(field) {
        tracing::warn!(
            target: "secall::llm_defaults",
            field,
            value,
            "config 의 {} 미설정 → \"{}\" 사용 (config 에 명시하면 이 경고가 사라집니다)",
            field,
            value
        );
    }
}
