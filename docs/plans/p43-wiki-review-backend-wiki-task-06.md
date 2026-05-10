---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 06
title: P42 review recommendations 적용
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 06 — P42 review recommendations 적용

## Changed files

수정:
- `crates/secall-core/src/graph/semantic.rs:274` — `pub async fn extract_with_gemini` → `pub(crate) async fn extract_with_gemini`. 외부 (다른 crate) 노출 차단.
- `crates/secall-core/src/graph/semantic.rs:417` — `pub async fn extract_with_llm` → `pub(crate) async fn extract_with_llm`.
- `crates/secall-core/tests/semantic_backends.rs` — `pub(crate)` 로 좁히면 integration test 가 import 못 함. integration test 를 `#[cfg(test)] mod` 로 옮기거나, 함수를 `pub(crate)` + `#[cfg(test)] pub` 로 dual gate. **선호**: `crates/secall-core/src/graph/semantic.rs` 에 `#[cfg(test)] pub use` 또는 `#[cfg(any(test, feature = "test-utils"))] pub` 패턴.
- `crates/secall/src/commands/config.rs:259-590` 의 `async fn test_backend` 함수를 backend 별 helper 로 분할. 신규: `test_backend_claude`, `test_backend_codex`, `test_backend_haiku`, `test_backend_ollama`, `test_backend_lmstudio`, `test_backend_gemini`. `test_backend` dispatcher 만 30줄 이내로 축소.
- `crates/secall-core/tests/rest_config.rs:241` — `assert!(saved.contains(r#"backend = "haiku""#))` → 정규식 `regex::Regex::new(r#"^backend = "haiku""#).is_match(&line)` 으로 라인 단위 검증. 또는 toml::from_str 으로 파싱 후 구조 비교.

신규: 없음 (수정만).

회귀 테스트:
- 본 task 의 변경은 모두 외부 동작 호환 — 기존 테스트가 그대로 통과해야 함. 신규 테스트 X.

## Change description

### 1. semantic.rs 가시성 축소 + test-friendly gate

```rust
// crates/secall-core/src/graph/semantic.rs

#[cfg(test)]
pub async fn extract_with_gemini(...) -> Result<Vec<GraphEdge>> { ... }

#[cfg(not(test))]
pub(crate) async fn extract_with_gemini(...) -> Result<Vec<GraphEdge>> { ... }
```

**문제**: integration test (`tests/semantic_backends.rs`) 는 `#[cfg(test)]` 가 적용 안 됨 (별도 crate 컴파일).

**대안 1** — `pub(crate)` 로 좁히고 integration test 를 unit test 로 이동 (`semantic.rs` 안의 `#[cfg(test)] mod tests`).
**대안 2** — `pub(crate)` 유지하되 `pub fn extract_with_llm_for_test(...)` 같은 wrapper 를 `#[cfg(any(test, feature = "test-utils"))]` 로 export.
**대안 3** — feature flag `test-utils` 도입 → CI 에서 enable.

**채택**: 대안 1 (단순함 + 외부 의존 0). `tests/semantic_backends.rs` 의 5 case 를 `crates/secall-core/src/graph/semantic.rs` 의 `#[cfg(test)] mod tests` 로 이동. mockito + tokio runtime 은 그대로.

이동 후 `tests/semantic_backends.rs` 파일 삭제.

### 2. test_backend 함수 분할

기존 (`crates/secall/src/commands/config.rs:259-590`):

```rust
async fn test_backend(config: &Config, backend: &str, no_network: bool) -> TestOutcome {
    match backend {
        "claude" => { /* 30+ lines */ }
        "codex" => { /* 30+ lines */ }
        "haiku" => { /* 50+ lines */ }
        "ollama" => { /* 40+ lines */ }
        "lmstudio" => { /* 40+ lines */ }
        "gemini" => { /* 50+ lines */ }
        _ => unreachable!(),
    }
}
```

신규:

```rust
async fn test_backend(config: &Config, backend: &str, no_network: bool) -> TestOutcome {
    match backend {
        "claude" => test_backend_claude(no_network).await,
        "codex" => test_backend_codex(no_network).await,
        "haiku" => test_backend_haiku(no_network).await,
        "ollama" => test_backend_ollama(config, no_network).await,
        "lmstudio" => test_backend_lmstudio(config, no_network).await,
        "gemini" => test_backend_gemini(config, no_network).await,
        _ => TestOutcome { backend: backend.into(), status: TestStatus::Fail, detail: "unknown backend".into() },
    }
}

async fn test_backend_claude(no_network: bool) -> TestOutcome { /* 기존 본문 그대로 */ }
async fn test_backend_codex(no_network: bool) -> TestOutcome { /* ... */ }
// ... 6개 helper
```

각 helper 는 `(config, no_network)` 만 받음 — 외부 의존 명시.

### 3. rest_config.rs 정규식 강화

기존 `tests/rest_config.rs:241`:

```rust
assert!(saved.contains(r#"backend = "haiku""#));
```

신규:

```rust
let parsed: toml::Value = toml::from_str(&saved).unwrap();
assert_eq!(
    parsed.get("log").and_then(|v| v.get("backend")).and_then(|v| v.as_str()),
    Some("haiku"),
    "expected [log].backend = \"haiku\" preserved, got: {saved}"
);
```

또는 정규식 (의도가 선명):

```rust
let re = regex::Regex::new(r#"(?m)^backend = "haiku"$"#).unwrap();
assert!(re.is_match(&saved), "expected log.backend line; got:\n{saved}");
```

> **선호**: toml::from_str 파싱 — false-positive 방지 (다른 섹션의 `backend = "haiku"` 와 구별 안 됨이 정규식의 한계).

`tests/rest_config.rs` 상단에 `use toml as toml_crate;` (이미 있을 가능성). 회귀 테스트 dep 추가 X.

### 4. 호환성 확인

- semantic.rs 의 `pub(crate)` 변경 — `crates/secall-core` 내부에서만 사용. `crates/secall` 등 외부 crate 가 직접 호출하면 컴파일 에러. grep 으로 외부 호출자 확인 후 이동.
- test_backend 의 helper 분할 — `private fn` 으로 둠. 외부 crate 노출 X.
- rest_config 정규식 — assertion 로직만 변경. 기존 패스 케이스 그대로 패스.

## Dependencies

- 의존 task 없음 — task 01–05 와 disjoint.
- crate dep: 없음 (mockito 는 P42 에서 이미 dev-dep).

## Verification

```bash
# 1. type / lint
cargo check -p secall-core
cargo check -p secall
cargo clippy --all-targets

# 2. semantic 분기 테스트 (이동 후 위치)
cargo test -p secall-core --lib graph::semantic::tests
cargo test -p secall-core --test semantic_backends 2>&1 | grep "no test target named" || true
# tests/semantic_backends.rs 가 삭제됐다면 위 명령은 실패 — 그게 의도

# 3. test_backend helper 분할 후 기존 테스트 회귀
cargo test -p secall --test config_llm_test

# 4. rest_config 정규식 강화 후 회귀
cargo test -p secall-core --test rest_config

# 5. (sanity) 전체 type 검사
cargo check --workspace
```

## Risks

- **integration test → unit test 이동의 부작용** — `cargo test --test semantic_backends` 명령이 실패하게 됨 (target 없음). CI 가 명시적으로 호출하면 갱신 필요. README / 회귀 doc 갱신 (task 07).
- **`#[cfg(test)] mod tests` 안의 import** — `use mockito::Server;` 등 dev-dep. Cargo.toml 의 `[dev-dependencies]` 에 mockito 가 이미 있으므로 OK.
- **test_backend helper 의 시그니처 정합** — 일부 helper 가 config 미사용 (e.g., claude/codex 는 PATH check 만). 시그니처 통일 위해 모두 `(&Config, bool)` 받되 unused warning 은 `#[allow(unused_variables)]`. clippy 의 `needless_pass_by_value` 경고 가능 — 본 task 는 가독성 우선.
- **rest_config 의 toml::from_str 비용** — 7 case 모두 한 번씩 파싱. 테스트 시간 영향 미미 (1ms 미만/case).
- **외부 crate 의 extract_with_* 호출자 0건 가정** — grep 으로 확인:
  ```bash
  grep -rn "extract_with_llm\|extract_with_gemini" crates/secall/ | grep -v test
  ```
  결과가 0건이어야 함. 있으면 task 03 의 dispatcher 로 통합 또는 별도 처리.

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/` — task 01 / 02 / 04 영역.
- `crates/secall/src/commands/wiki.rs` — task 03 영역.
- `crates/secall-core/src/vault/config.rs` 의 `Config::save` — task 05 영역.
- `crates/secall-core/src/graph/semantic.rs` 의 함수 본문 — 가시성 / `#[cfg(test)] mod` 이동 외 변경 X.
- `crates/secall/src/commands/config.rs` 의 `run_llm_test` / `run_show` 등 — 본 task 는 `test_backend` 분할만. 다른 함수 변경 X.
- `crates/secall-core/tests/rest_config.rs` 의 다른 6 case — 본 task 는 1 case 의 assertion 만 변경.
