use std::time::Duration;

/// kill_on_drop(true) 로 spawn 한 tokio 자식 프로세스가
/// handle 이 drop 될 때 실제로 종료되는지 검증한다.
/// ClaudeBackend / CodexBackend 비동기 전환의 핵심 보장.
#[tokio::test]
async fn test_async_child_killed_on_drop() {
    use std::process::Stdio;

    let child = tokio::process::Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("sleep 명령이 PATH 에 있어야 함");

    let pid = child.id().expect("pid 를 읽을 수 있어야 함");

    drop(child);

    tokio::time::sleep(Duration::from_millis(300)).await;

    // kill -0 은 프로세스가 존재하면 exit 0, 없으면 non-zero
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("kill -0 실행 가능해야 함")
        .success();

    assert!(
        !alive,
        "kill_on_drop 으로 spawn 된 자식은 handle drop 시 종료되어야 함"
    );
}

/// ClaudeBackend::generate 가 claude CLI 미설치 시 즉시 에러를 반환하고
/// tokio runtime 을 block 하지 않는지 검증한다.
#[tokio::test]
async fn test_claude_backend_fails_fast_when_not_found() {
    use secall_core::wiki::{ClaudeBackend, WikiBackend as _};

    // claude 가 설치되어 있으면 이 테스트는 skip (manual smoke 로 대체)
    if secall_core::command_exists("claude") {
        return;
    }

    let backend = ClaudeBackend {
        model: "sonnet".to_string(),
        vault_path: std::env::temp_dir(),
        timeout_secs: 10,
    };

    let result = tokio::time::timeout(Duration::from_secs(5), backend.generate("test")).await;

    // timeout 이 아닌 즉시 에러여야 한다
    assert!(
        result.is_ok(),
        "generate() 가 5초 내에 완료(에러 포함)되어야 함"
    );
    assert!(
        result.unwrap().is_err(),
        "claude CLI 없으면 에러를 반환해야 함"
    );
}

/// CodexBackend::generate 가 codex CLI 미설치 시 즉시 에러를 반환하는지 검증.
#[tokio::test]
async fn test_codex_backend_fails_fast_when_not_found() {
    use secall_core::wiki::{CodexBackend, WikiBackend as _};

    if secall_core::command_exists("codex") {
        return;
    }

    let backend = CodexBackend {
        model: "gpt-5.4".to_string(),
        vault_path: std::env::temp_dir(),
        timeout_secs: 10,
    };

    let result = tokio::time::timeout(Duration::from_secs(5), backend.generate("test")).await;

    assert!(
        result.is_ok(),
        "generate() 가 5초 내에 완료(에러 포함)되어야 함"
    );
    assert!(
        result.unwrap().is_err(),
        "codex CLI 없으면 에러를 반환해야 함"
    );
}
