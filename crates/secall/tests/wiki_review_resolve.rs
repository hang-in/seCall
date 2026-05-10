use secall::commands::wiki::{resolve_review_backend, resolve_review_model};
use secall_core::vault::Config;

#[test]
fn review_backend_priority() {
    let mut config = Config::default();
    config.wiki.default_backend = "ollama".into();
    config.wiki.review_backend = Some("claude".into());

    assert_eq!(resolve_review_backend(Some("haiku"), &config), "haiku");
    assert_eq!(resolve_review_backend(None, &config), "claude");

    config.wiki.review_backend = None;
    assert_eq!(resolve_review_backend(None, &config), "ollama");

    config.wiki.default_backend = "non-existent-backend".into();
    assert_eq!(resolve_review_backend(None, &config), "haiku");
}

#[test]
fn review_model_defaults_follow_backend() {
    let mut config = Config::default();

    assert_eq!(
        resolve_review_model(None, &config, "haiku"),
        "claude-haiku-4-5-20251001"
    );
    assert_eq!(resolve_review_model(None, &config, "codex"), "gpt-5.4");

    config.graph.ollama_model = Some("qwen2.5:14b".into());
    assert_eq!(
        resolve_review_model(None, &config, "ollama"),
        "qwen2.5:14b"
    );
}
