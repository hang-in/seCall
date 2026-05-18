---
type: plan
status: in_progress
updated_at: 2026-05-19
canonical: true
---

# P82 — `Config::save()` integration test 가드 확장

## 배경

2026-05-16 사고: `cargo test --lib -p secall-core vault::` flaky test 재현 시도 (P58 race fix 머지 전) 중 production `~/Library/Application Support/secall/config.toml` 의 `[vault].path` 가 unit test `save_preserves_top_level_comments` 의 hardcoded 값 (`/tmp/changed`) 으로 덮어쓰여, 사용자 web UI 가 wiki 빈 화면 / graph 멈춤. 복구 후 P68 (`#[cfg(test)]` 가드) 도입.

**한계**: `#[cfg(test)]` 는 컴파일 시점 — `secall-core` 가 lib 으로 컴파일될 때 (integration test 입장의 외부 dependency) false. 따라서 `crates/secall-core/tests/*.rs` 에서 `secall_core::vault::Config::save()` 직·간접 호출 시 가드 무력. core-backlog hot 1건으로 박제됨.

## 목표

- `Config::save()` 가 unit test + integration test 두 컨텍스트 모두에서 `SECALL_CONFIG_PATH` 미설정 시 production config 를 덮어쓰지 못하도록 보호.
- 사용자 직접 사고 (2026-05-16) 의 재발 차단.

## 비목표

- production `secall` CLI 동작 변경 없음 (env 미설정 시 정상 save).
- `commands/config.rs`, `commands/init.rs` 의 save 호출 사이트 수정 없음.
- core-backlog 의 다른 항목 (dist rerun-if-changed, /api/status 분리 등) 은 별도 P 번호.

## 옵션 비교 (요약)

| 옵션 | 장점 | 단점 | 선택 |
|---|---|---|---|
| **A. Runtime env guard** (`SECALL_TEST_MODE=1`) | integration test 까지 보호. 변경 최소. unit test 가드 패턴 그대로 재활용 | 명시적 env set 필요 (자동 검출 불가능) | ✅ |
| B. `save_to_path(&path)` helper 분리 | API 명시 | 호출 사이트 다수 수정. 가드 자체 문제는 미해결 | ❌ |
| C. `serial_test` crate | race 직렬화 | 가드와 직교 (이미 ENV_MUTEX 존재). 사고 직접 방지 아님 | ❌ |

## 구현 절차

### 1. `crates/secall-core/src/vault/config.rs` 의 `Config::save()` 가드 통합

- 기존 `#[cfg(test)]` 블록을 `cfg!(test) || SECALL_TEST_MODE` runtime check 로 확장.
- error message 에 `SECALL_TEST_MODE` 도 언급.

### 2. `crates/secall-core/tests/common/mod.rs` 에 `ensure_test_mode()` 추가

- `std::sync::Once` 로 1회 set. 모듈 사용처가 호출.
- env unset 은 안 함 (test 종료 시까지 살아 있도록).

### 3. `Config::save()` 가 (직·간접) 호출될 수 있는 integration tests 의 setup 에 `common::ensure_test_mode()` 호출 추가

- `rest_config.rs` — 모든 test fn 의 ENV_MUTEX 잡은 직후
- `vault_auto_commit.rs` — Config 사용 여부 grep 검증
- 그 외 `secall_core::vault::Config` import 한 test 파일 검증

### 4. 신규 unit test `save_refuses_in_runtime_test_context_without_env`

- ENV_MUTEX 잡기 → `SECALL_TEST_MODE=1` 보장 → `SECALL_CONFIG_PATH` remove → `Config::default().save()` → err 검증 → env 복원.

### 5. `docs/reference/core-backlog.md` 의 hot 1건 해소 표기

- 해당 entry 에 "해소: PR #N (P82)" 표시 또는 hot → done 이동.

## 변경 파일

| 파일 | 변경 종류 |
|---|---|
| `crates/secall-core/src/vault/config.rs` | save() 가드 통합 + 신규 unit test 1건 |
| `crates/secall-core/tests/common/mod.rs` | `ensure_test_mode()` 추가 |
| `crates/secall-core/tests/rest_config.rs` | setup 에 `common::ensure_test_mode()` 호출 (각 test fn 또는 모듈 상단 1회) |
| `crates/secall-core/tests/vault_auto_commit.rs` | Config 사용 시 동일 |
| `docs/reference/core-backlog.md` | hot 항목 해소 표기 |
| `docs/plans/index.md` | P82 plan 등록 |

## 검증

```bash
cargo test -p secall-core vault::config::tests
cargo nextest run -p secall-core --test rest_config --test vault_auto_commit
cargo nextest run --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## 리스크

- integration test 누락 시 새 가드에 걸려 fail → 변경 후 워크스페이스 nextest 로 회귀 검증 필수.
- `SECALL_TEST_MODE` env 가 CI/dev shell 에 leak 시 production CLI 사용 시 의도치 않은 가드 trigger.
  - CI: test 만 실행하므로 OK
  - 개발자 shell: 명시적 unset 필요 — handoff 문서에 명시.
