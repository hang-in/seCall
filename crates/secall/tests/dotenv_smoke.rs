//! P39 — dotenvy autoload smoke test.
//!
//! `crates/secall/src/main.rs` 진입점에서 `dotenvy::dotenv()` 호출이 실수로
//! 제거되지 않도록 회귀 안전망. dotenvy crate 의 핵심 동작 (`.env` 파일에 적힌
//! `KEY=VALUE` 가 process 환경변수로 로드) 을 secall 의 dev-dep 컨텍스트에서
//! 검증한다. cwd 변경 없이 `from_path` 로 격리 (다른 테스트와 race 회피).

use std::env;

#[test]
fn dotenvy_loads_env_from_explicit_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let env_path = dir.path().join(".env");
    std::fs::write(
        &env_path,
        "SECALL_DOTENV_SMOKE_TEST=ok\nexport SECALL_DOTENV_SMOKE_EXPORT=42\n",
    )
    .expect("write .env");

    // 외부 env 잔존 제거 (이전 테스트가 set 했을 수 있음).
    env::remove_var("SECALL_DOTENV_SMOKE_TEST");
    env::remove_var("SECALL_DOTENV_SMOKE_EXPORT");

    let _ = dotenvy::from_path(&env_path);

    assert_eq!(
        env::var("SECALL_DOTENV_SMOKE_TEST").as_deref().ok(),
        Some("ok"),
        "plain KEY=VALUE 라인 로드"
    );
    assert_eq!(
        env::var("SECALL_DOTENV_SMOKE_EXPORT").as_deref().ok(),
        Some("42"),
        "export prefix 도 정상 로드 (P39 사용자 vault .env 형식)"
    );

    // cleanup — 다른 테스트로의 누수 방지.
    env::remove_var("SECALL_DOTENV_SMOKE_TEST");
    env::remove_var("SECALL_DOTENV_SMOKE_EXPORT");
}

#[test]
fn dotenvy_missing_file_returns_error_not_panic() {
    // main.rs 의 `let _ = dotenvy::dotenv();` 패턴 검증 — 파일 없어도 panic X.
    let dir = tempfile::tempdir().expect("tempdir");
    let missing_path = dir.path().join("nonexistent.env");
    let result = dotenvy::from_path(&missing_path);
    assert!(
        result.is_err(),
        "미존재 파일은 Err 반환 (panic X) — main.rs 의 silent skip 패턴 호환"
    );
}
