use std::process::Command;

fn secall_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secall"))
}

#[test]
fn config_llm_test_no_network_runs_offline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"
"#,
    )
    .expect("write config");

    let output = secall_cmd()
        .args(["config", "llm", "test", "--no-network"])
        .env("SECALL_CONFIG_PATH", &config_path)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("SECALL_GEMINI_API_KEY")
        .output()
        .expect("run secall");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[ollama"));
    assert!(stdout.contains("[lmstudio"));
    assert!(stdout.contains("[claude"));
    assert!(stdout.contains("[codex"));
    assert!(stdout.contains("[haiku"));
    // [gemini] 백엔드는 P46 에서 제거되어 더 이상 stdout 에 노출되지 않음.
    assert!(stdout.contains("haiku"));
    assert!(stdout.contains("FAIL ANTHROPIC_API_KEY not set"));
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn config_llm_test_unknown_backend_errors() {
    let output = secall_cmd()
        .args(["config", "llm", "test", "foo"])
        .output()
        .expect("run secall");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown backend"));
}

#[test]
fn config_llm_test_lmstudio_uses_graph_ollama_url_in_no_network_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[graph]
semantic_backend = "lmstudio"
ollama_url = "http://localhost:1234"
"#,
    )
    .expect("write config");

    let output = secall_cmd()
        .args(["config", "llm", "test", "lmstudio", "--no-network"])
        .env("SECALL_CONFIG_PATH", &config_path)
        .output()
        .expect("run secall");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[lmstudio] OK"));
    assert!(stdout.contains("http://localhost:1234"));
}
