use secall::commands::log::{resolve_backend_name, resolve_log_model};
use secall_core::{
    llm::defaults::{LOG_GEMINI_DEFAULT, LOG_OLLAMA_DEFAULT},
    vault::Config,
};

#[test]
fn backend_resolution_priority_matches_plan() {
    let mut config = Config::default();
    config.log.backend = Some("claude".to_string());
    config.graph.semantic_backend = "gemini".to_string();
    assert_eq!(resolve_backend_name(&config, Some("haiku")), "haiku");
    assert_eq!(resolve_backend_name(&config, None), "claude");

    config.log.backend = None;
    assert_eq!(resolve_backend_name(&config, None), "gemini");

    config.graph.semantic_backend.clear();
    assert_eq!(resolve_backend_name(&config, None), "ollama");
}

#[test]
fn model_resolution_priority_matches_plan() {
    let mut config = Config::default();
    config.log.model = Some("config-model".to_string());

    assert_eq!(
        resolve_log_model(&config, "ollama", Some("cli-model")).as_deref(),
        Some("cli-model")
    );
    assert_eq!(
        resolve_log_model(&config, "ollama", None).as_deref(),
        Some("config-model")
    );

    config.log.model = None;
    assert_eq!(
        resolve_log_model(&config, "ollama", None).as_deref(),
        Some(LOG_OLLAMA_DEFAULT)
    );
    assert_eq!(
        resolve_log_model(&config, "gemini", None).as_deref(),
        Some(LOG_GEMINI_DEFAULT)
    );
    assert_eq!(resolve_log_model(&config, "claude", None), None);
    assert_eq!(resolve_log_model(&config, "codex", None), None);
    assert_eq!(resolve_log_model(&config, "lmstudio", None), None);
}
