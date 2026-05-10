---
type: task
plan_slug: p42-p41-llm
task_id: 03
title: `secall config llm test` 구현
parallel_group: A
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 03 — `secall config llm test` 구현

## Changed files

수정:
- `crates/secall/src/main.rs:292` — `LlmAction` enum 에 신규 variant `Test { backend: Option<String>, no_network: bool }` 추가.
- `crates/secall/src/main.rs:674` — `ConfigAction::Llm` match 에 `LlmAction::Test { backend, no_network } => commands::config::run_llm_test(backend, no_network).await?` 라인 추가.
- `crates/secall/src/commands/config.rs:46` 근처 — 신규 `pub async fn run_llm_test(backend: Option<String>, no_network: bool) -> Result<()>` 함수.
- `crates/secall/src/commands/config.rs` 상단 — 필요 시 `use secall_core::wiki::{...}` 또는 `secall_core::llm::defaults::*` 추가.

신규:
- `crates/secall/tests/config_llm_test.rs` (신규) — `secall config llm test --no-network` 의 stdout 회귀 + exit code 회귀.

> **main 의 ConfigAction::Llm async** 처리: 현재 `run_llm_show / run_llm_set / run_llm_where` 가 sync. async 추가 시 `match action` 의 arm 만 await 하면 됨 (main.rs 의 ConfigAction 처리는 이미 async fn 안).

## Change description

### 1. CLI 시그니처

```
secall config llm test [<backend>] [--no-network]

  <backend>      claude / codex / haiku / ollama / lmstudio / gemini.
                 생략 시 모든 backend 순차 검증.
  --no-network   실제 네트워크 호출 skip — 인증 자체 (API key / CLI 존재 여부) 만 검증. CI 용.
```

### 2. backend 별 ping 정의

| backend | check (`--no-network` 가 아닐 때) | `--no-network` 일 때 |
|---|---|---|
| claude | `claude --version` (subprocess, 1s timeout) | `which claude` 또는 PATH check |
| codex | `codex --version` | `which codex` |
| haiku | `ANTHROPIC_API_KEY` env + 1-token messages call | env 존재 여부만 |
| ollama | `GET {ollama_url}/api/tags` (200 OK) | `ollama_url` config 설정 여부 |
| lmstudio | `GET {api_url}/v1/models` (200 OK) | `api_url` 설정 여부 |
| gemini | `GEMINI_API_KEY` env + 짧은 generateContent (1-token) | env 또는 `gemini_api_key` 존재 여부 |

각 결과는 `OK` / `FAIL <reason>` / `SKIP <reason>` 으로 1줄 출력:

```
$ secall config llm test
[ollama]   OK    http://localhost:11434  (gemma4:e4b)
[claude]   OK    /usr/local/bin/claude   (claude --version 출력)
[codex]    FAIL  not installed (PATH 에 codex 없음)
[haiku]    OK    ANTHROPIC_API_KEY set, 1-token call 200
[lmstudio] SKIP  api_url not configured
[gemini]   OK    SECALL_GEMINI_API_KEY set, 1-token call 200
```

`FAIL` 이 1개 이상이면 exit code = 2. 모두 OK / SKIP 이면 0.

### 3. 단일 backend 호출

```
$ secall config llm test ollama
[ollama]   OK    http://localhost:11434  (gemma4:e4b)
```

backend 미지원 이름이면 exit 1 + `unknown backend: foo. valid: claude/codex/haiku/ollama/lmstudio/gemini`.

### 4. 구현 가드라인

- **timeout**: 각 ping 5초. 누적 30초 초과 시 stderr warning.
- **외부 의존 없는 검증**: subprocess (`claude --version`) 는 `tokio::process::Command` 로 비동기 실행.
  실패 시 stderr 의 첫 200자만 출력 (긴 stack trace 차단).
- **secret 노출 금지**: API key 자체는 stdout 에 절대 출력 X. "set" / "not set" 만.
- **Ollama API 검증**: `GET /api/tags` 응답에 `models` 배열 존재 + `ollama_model` 이 그 안에 있는지 (선택) 확인.
- **Anthropic / Gemini 1-token call**: prompt `"hi"`, `max_tokens=1`. 200 OK + JSON parsing 성공이면 OK.
  비용은 사실상 0 (1 token).

### 5. 회귀 테스트

`tests/config_llm_test.rs`:

```rust
use std::process::Command;

fn secall_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_secall"))
}

#[test]
fn config_llm_test_no_network_runs_offline() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, r#"[vault]
path = "/tmp/test-vault"
"#).unwrap();

    let output = secall_cmd()
        .args(["config", "llm", "test", "--no-network"])
        .env("SECALL_CONFIG_PATH", &config_path)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("SECALL_GEMINI_API_KEY")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // 6 backend 각각 1줄 출력
    assert!(stdout.contains("[ollama]"));
    assert!(stdout.contains("[lmstudio]"));
    assert!(stdout.contains("[claude]"));
    assert!(stdout.contains("[codex]"));
    assert!(stdout.contains("[haiku]"));
    assert!(stdout.contains("[gemini]"));
    // env 미설정 → haiku / gemini 가 FAIL
    assert!(stdout.contains("[haiku]   FAIL") || stdout.contains("[haiku]    FAIL"));
    // exit code 2 (FAIL 존재)
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn config_llm_test_unknown_backend_errors() {
    let output = secall_cmd()
        .args(["config", "llm", "test", "foo"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown backend"));
}
```

## Dependencies

- 의존 task 없음. `LlmAction::Show / Set / Where` 는 P41 task 05 에서 이미 추가.
- crate dep: 추가 없음. `tokio::process`, `reqwest` 모두 기존 dep.

## Verification

```bash
# 1. type / lint
cargo check -p secall
cargo clippy --all-targets -p secall

# 2. CLI help
./target/debug/secall config llm test --help | grep -E "backend|no-network"

# 3. 회귀 테스트 (오프라인)
cargo test -p secall --test config_llm_test

# 4. (수동) 실제 ping
./target/debug/secall config llm test ollama
./target/debug/secall config llm test claude
./target/debug/secall config llm test --no-network    # CI 시나리오

# 5. exit code
./target/debug/secall config llm test --no-network; echo "exit=$?"
```

## Risks

- **subprocess 의존** — claude / codex CLI 가 PATH 에 없을 때 timeout 안에 friendly fail.
  `tokio::process::Command::output()` 의 `kill_on_drop(true)` 사용.
- **1-token call 의 비용** — 사실상 무료 (Anthropic 의 input 토큰 1 + output 1 = $0.00).
  과도한 ping 으로 rate limit trip 가능 — `secall config llm test` 의 도움말에 "1회당 1 token 호출" 명시.
- **Gemini API endpoint** — `extract_with_gemini` 와 동일 endpoint 재사용. URL 변경 시 task 02 와 함께 갱신.
- **flaky 외부 의존** — CI 에서는 항상 `--no-network` 사용. 본 task 의 unit test 는 `--no-network` 만 검증.
- **누적 timeout** — 6 backend 순차 검증 시 최악 30초. 본 task 는 sequential — 향후 parallel 검토.

## Scope boundary (수정 금지)

- `crates/secall/src/commands/log.rs` — task 01 영역.
- `crates/secall-core/src/graph/semantic.rs` — task 02 영역.
- `crates/secall-core/src/mcp/` — task 05 영역.
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — backend impl 변경 X.
- `crates/secall-core/src/vault/config.rs` — schema 변경 X.
- `web/` — task 04 영역.
