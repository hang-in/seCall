use std::process::Command;

fn secall_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secall"))
}

#[test]
fn config_llm_show_prints_expected_sections() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        r#"
[vault]
path = "/tmp/test-vault"

[log]
backend = "haiku"
"#,
    )
    .expect("write config");

    let output = secall_cmd()
        .args(["config", "llm", "show"])
        .env("SECALL_CONFIG_PATH", &config_path)
        .output()
        .expect("run secall");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Wiki"));
    assert!(stdout.contains("Graph"));
    assert!(stdout.contains("Log"));
    assert!(stdout.contains("Embedding"));
    assert!(stdout.contains("Environment indicators"));
}

#[test]
fn config_llm_set_updates_log_backend() {
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
        .args(["config", "llm", "set", "log.backend", "haiku"])
        .env("SECALL_CONFIG_PATH", &config_path)
        .output()
        .expect("run secall");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let saved = std::fs::read_to_string(&config_path).expect("read config");
    assert!(saved.contains("backend = \"haiku\""));
}
