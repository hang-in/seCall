//! P82: integration test 환경에서 `Config::save()` 의 runtime 가드가
//! production config 를 덮어쓰지 못하게 차단하는지 검증.
//!
//! - cfg!(test) 는 integration test crate 입장에서 false (lib 컴파일 시점은 false).
//! - 대신 `common::ensure_test_mode()` 가 `SECALL_TEST_MODE=1` 을 set 하므로
//!   save() 의 runtime 가드 (`cfg!(test) || SECALL_TEST_MODE`) 가 trigger.

mod common;

use std::sync::Mutex;

use secall_core::vault::Config;

/// env 조작 직렬화. integration test binary 의 모든 thread 가 공유.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn save_refuses_in_integration_test_without_secall_config_path() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    common::ensure_test_mode();
    // 다른 test 의 잔여 set 이 있을 수 있어 명시적 unset.
    std::env::remove_var("SECALL_CONFIG_PATH");

    let config = Config::default();
    let result = config.save();

    assert!(
        result.is_err(),
        "save() must refuse without SECALL_CONFIG_PATH in integration test \
         context (SECALL_TEST_MODE set, cfg!(test) false)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("SECALL_CONFIG_PATH"),
        "error must mention SECALL_CONFIG_PATH, got: {msg}"
    );
    assert!(
        msg.contains("SECALL_TEST_MODE") || msg.contains("test"),
        "error should hint at test context detection, got: {msg}"
    );
}

#[test]
fn save_succeeds_when_secall_config_path_is_tempdir() {
    let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    common::ensure_test_mode();
    let tempdir = tempfile::tempdir().expect("create tempdir");
    let config_path = tempdir.path().join("config.toml");
    std::env::set_var("SECALL_CONFIG_PATH", &config_path);

    let config = Config::default();
    let result = config.save();

    std::env::remove_var("SECALL_CONFIG_PATH");

    assert!(
        result.is_ok(),
        "save() must succeed with SECALL_CONFIG_PATH set to tempdir, got: {:?}",
        result.err()
    );
    assert!(
        config_path.exists(),
        "config.toml should be written to tempdir"
    );
}
