---
type: subtask
status: draft
updated_at: 2026-05-12
plan_id: P46
task_id: 01
parallel_group: 1
depends_on: []
---

# P46 Task 01 — secall sync 미종료 진단 + fix

## Changed files

### 진단 대상 (읽기 + 원인 파악)

- `crates/secall/src/commands/sync.rs:49-67` — `pub async fn run` (CLI 진입점)
- `crates/secall/src/commands/sync.rs:72-` — `run_with_progress` (실제 본체)
- `crates/secall/src/commands/sync.rs:287-351` — Phase 3.5 wiki update 루프 (사용자가 "위키 다 만들고 종료 안 됨" 이라 보고한 지점)
- `crates/secall/src/commands/wiki.rs:131-` — `run_update` (Phase 3.5 에서 세션마다 호출)
- `crates/secall-core/src/wiki/claude.rs:34-71` — `ClaudeBackend::generate` (`std::process::Command` 로 `claude` CLI spawn → `wait()`)
- `crates/secall-core/src/wiki/codex.rs:13-` — `CodexBackend::generate` (`codex` CLI spawn)
- `crates/secall-core/src/wiki/reviewers/claude.rs:49-` — `tokio::process::Command::new(bin)` (review 단계의 claude CLI)
- `crates/secall-core/src/wiki/reviewers/codex.rs:32-` — review 단계의 codex CLI
- `crates/secall/src/main.rs:451` — `#[tokio::main]` (runtime 종료 흐름)

### 수정 대상 (진단 결과에 따라 변경)

진단 후 원인이 좁혀지면 다음 중 해당 파일에서 수정:
- `crates/secall-core/src/wiki/claude.rs` 또는 `codex.rs` — child stdout 읽기 중간 break 후 `child.wait()` 미호출 / stderr drain 누락 등.
- `crates/secall-core/src/wiki/reviewers/claude.rs` 또는 `codex.rs` — `tokio::process::Command` 의 child handle drop 시 행동 (`kill_on_drop` 미설정).
- `crates/secall/src/commands/sync.rs` — wiki update 루프 종료 후 명시적 drop / explicit `tokio::join!` 누락.
- `crates/secall/src/commands/wiki.rs` — 내부에서 spawn 한 background task 미 await.

### 신규 회귀 테스트

- `crates/secall-core/tests/sync_termination.rs` (신규) — `secall sync` 실행 후 N 초 안에 프로세스가 종료되는지 검증하는 통합 테스트. wiki update 가 실제 외부 CLI 를 호출하지 않도록 mock backend 사용 (또는 `--no-wiki` 플래그로 wiki 우회 + wiki 호출 path 단위 테스트는 별도).

## Change description

### 진단 절차 (Developer 가 따라야 할 순서)

1. **재현** — `cargo run -p secall -- sync` 로 sync 를 끝까지 돌리고, wiki update phase 완료 후 프로세스가 살아있는지 `ps` / `pgrep secall` 로 확인. 살아있다면 stuck 위치 파악을 위해 `kill -SIGQUIT <pid>` 로 backtrace 또는 `dtruss -p <pid>` 로 시스템 콜 확인.

2. **원인 후보 좁히기** — 다음 중 하나일 가능성이 높음 (Developer 가 확인 후 좁히기):
   - (a) `ClaudeBackend::generate` 가 `claude` CLI 의 stdout 을 다 읽기 전에 stderr 가 막혀서 child 가 안 끝남.
   - (b) `tokio::process::Command` 로 spawn 한 reviewer child 가 `kill_on_drop(false)` 기본값이라 await 누락 시 좀비.
   - (c) `reqwest::Client` 의 connection pool keep-alive 가 tokio runtime 종료 시 idle connection drop 지연 (보통 0~30s, "계속 차감"과는 시간 스케일 불일치 → 가능성 낮음).
   - (d) `#[tokio::main]` 매크로의 runtime drop 이 spawn 된 detached task 를 무한 대기 (sync 안에서 `tokio::spawn` 한 부분이 await 되지 않았을 가능성).

3. **수정 방향** — 원인이 (a)/(b) 라면 child handle 의 `wait()` 또는 `.kill_on_drop(true)` 명시. 원인이 (d) 라면 누락된 `.await` 또는 `JoinHandle` 수집 추가. 사용자가 "차감이 계속 된다" 라고 보고했으므로 **(a) 또는 (b) 가 가장 유력** — claude/codex CLI 가 좀비로 남아 Anthropic 사용량을 계속 청구.

4. **회귀 테스트 작성** — `tests/sync_termination.rs` 에 다음 시나리오 추가:
   - `secall sync --no-wiki --no-semantic --no-graph --no-embed --local-only` 가 `Result<()>` 반환 후 명확히 종료되는지 (subprocess 가 살아있지 않은지) 검증.
   - 가능하면 wiki backend 호출 후에도 동일 검증 (mock backend 가 어렵다면 manual 검증 단계로 명시).

### 구현 제약

- **추측 수정 금지** — 원인을 진단 없이 임시 패치 (`std::process::exit(0)` 강제 종료 같은 거) 로 덮지 말 것. 좀비 subprocess 가 남으면 다음 sync 에서 같은 문제 재발.
- 외부 CLI subprocess 의 stdout/stderr 는 모두 read-to-end 후 `wait()` 호출. stdin 은 명시적으로 drop.
- `tokio::process::Command` 사용 시 `.kill_on_drop(true)` 기본값으로 설정 (handle drop 되면 자동 kill).

## Dependencies

없음. Task 02 (Gemini 제거) 와 영역이 분리되어 있어 병렬 실행 가능 (parallel_group 1).

## Verification

```bash
# 단위 테스트: wiki backend 의 subprocess wait 동작
cargo test -p secall-core --lib wiki::claude
cargo test -p secall-core --lib wiki::codex
cargo test -p secall-core --lib wiki::reviewers

# 신규 통합 테스트: sync 종료 검증
cargo test -p secall-core --test sync_termination

# 실제 재현 (manual): wiki update 후 프로세스 즉시 종료 확인
cargo build -p secall --release
timeout 600 ./target/release/secall sync --local-only
echo "exit_code=$?"
# Manual: exit_code=0 이고 추가 backgound process 없음을 확인
# Manual: ps aux | grep -E '(secall|claude|codex)' 출력이 grep 자기 자신만 나와야 함
```

## Risks

- **외부 CLI 종료 행동 변경** — `kill_on_drop(true)` 로 바꾸면 CLI 가 중간에 강제 종료될 수 있어, 진행 중인 claude/codex 응답이 잘릴 수 있음. wiki update loop 의 cancel path 와 충돌하지 않게 검토 필요.
- **회귀 테스트 환경 의존성** — `claude` / `codex` CLI 가 CI 환경에 없으므로 통합 테스트는 mock 또는 `--no-wiki` path 로만 검증. 진짜 wiki backend 의 종료 동작은 manual smoke 로 보완.
- **사용자 사용량 차감 손해 누적** — 진단/fix 가 늦어질수록 사용자 비용 손실 누적. Developer 는 가능한 한 빠르게 좀비 subprocess 가 원인인지부터 확인.
- code-review-graph 상 `crates/secall/src/commands/sync.rs` 변경은 영향 함수 다수 (`run`, `run_with_progress`, `run_auto_ingest`). 변경 범위 최소화.

## Scope boundary (수정 금지)

- `crates/secall-core/src/ingest/gemini.rs`, `gemini_web.rs`, `detect.rs::find_gemini_sessions` — Task 02 와도 무관, 외부 세션 ingest 코드.
- `crates/secall-core/src/graph/semantic.rs` — Task 02 영역 (Gemini 호출 제거).
- `crates/secall-core/src/vault/config.rs` 의 `[graph]`, `[log]` 섹션 schema — Task 02/03 영역.
- `crates/secall/src/commands/log.rs` 의 backend dispatch — Task 02/04 영역.
- `web/src/routes/SettingsRoute.tsx` — Task 02/05 영역.
