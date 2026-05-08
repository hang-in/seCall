use secall::commands::log::resolve_backend_name;
use secall_core::vault::Config;

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
