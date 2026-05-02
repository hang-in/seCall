---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p36-job-cancellation
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — CancellationToken 인프라 (registry + executor + REST)

## Changed files

수정:
- `Cargo.toml` — `tokio-util` workspace dep 확인 (`features = ["rt"]`). 이미 `tokio-util = { version = "0.7" }` 가 있으므로 default features 로 `CancellationToken` 사용 가능 — features 추가 없으면 그대로 두고 디벨로퍼가 컴파일 시 확인.
- `crates/secall-core/Cargo.toml` — 변경 없음 (`tokio-util.workspace = true` 이미 있음, 확인만)
- `crates/secall-core/src/jobs/mod.rs:29-33` — `ProgressSink` trait 에 cancel 폴링용 메서드 추가. trait method 1개 (sync, &self, bool 반환), default impl 은 `false` 로 후방 호환.
- `crates/secall-core/src/jobs/mod.rs:40-67` — `BroadcastSink` 에 `CancellationToken` 필드 추가 + 새 필드 받는 생성자 시그니처. `is_cancelled` 구현은 token 위임.
- `crates/secall-core/src/jobs/registry.rs:18-30` — `RegistryInner` 에 `cancel_tokens: HashMap<String, CancellationToken>` 필드 추가.
- `crates/secall-core/src/jobs/registry.rs:44` — `register` 시그니처 확장: `register(state, cancel_token)` — 호출처 1곳(executor.rs)만 영향.
- `crates/secall-core/src/jobs/registry.rs` (신규 메서드 3개):
  - `cancel(&self, id: &str) -> bool` — 해당 id 의 token cancel + 상태 즉시 `JobStatus::Interrupted` 전이. 미등록 → false. 이미 종료된 job → idempotent true.
  - `token_for(&self, id: &str) -> Option<CancellationToken>` — 외부(테스트) 조회용.
  - `evict` 본문에 `cancel_tokens.remove(id)` 한 줄 추가.
- `crates/secall-core/src/jobs/executor.rs:62-184` — `try_spawn` 본문에 다음 통합:
  1. spawn 직전에 `CancellationToken::new()` 생성, registry.register 와 BroadcastSink::new 둘 다에 전달
  2. spawned task 안에서 사용자 클로저 호출을 `tokio::select!` 로 token.cancelled() 와 race
  3. 완료 처리 분기에서 `cancel_token.is_cancelled()` 가 true 면 status 를 `Interrupted` 로 강제, error 메시지는 `"cancelled by user"`
  4. final broadcast event 도 cancelled 분기 시 `ProgressEvent::Failed { error, partial_result: None }` 로 발행 (partial_result 보존은 Task 01 어댑터가 outcome JSON 안에 채움)
- `crates/secall-core/src/mcp/rest.rs:667-678` — `api_cancel_job` 의 NOT_IMPLEMENTED stub 본문 교체:
  - `executor.registry.cancel(&id).await` 호출
  - true → 200 + `{ "cancelled": true, "job_id": ... }`
  - false → 404 + `{ "error": "job not found or already evicted" }`
- `crates/secall/src/commands/mod.rs:24-30` — `NoopSink` 의 `ProgressSink` impl 에 `is_cancelled` 메서드 명시 (default false 사용 가능하지만 명시 권장 — CLI 컨텍스트 의도 분명히).

신규: 없음

## Change description

### 핵심 설계

cancellation 신호 전달 경로:
```
REST POST /api/jobs/{id}/cancel
  → executor.registry.cancel(id)
    → token.cancel() + state.status = Interrupted
      → executor의 spawned task 안 select! 가 token.cancelled() 분기 진입
      → 동시에 어댑터(Task 01)가 sink.is_cancelled() 폴링하면서 안전 지점에서 자발적 종료
```

두 채널 모두 안전:
- `select!` 채널 — 어댑터가 외부 API 행 같은 곳에서 멈춰 있어도 강제로 future drop (단, drop 시 어댑터의 in-flight 작업은 어댑터가 자체적으로 cleanup 책임)
- `is_cancelled()` 폴링 채널 — 어댑터가 안전 지점마다 자발적 cancel + partial_outcome 반환 (Task 01 영역)

### Trait 시그니처 계약

`ProgressSink` 에 추가되는 단일 메서드:
- 이름: `is_cancelled`
- 시그니처: `fn is_cancelled(&self) -> bool` (sync, &self)
- default: `false`
- 호출 비용: cheap (atomic load 한 번) — 어댑터 hot loop 에서 자유롭게 호출 가능

### REST API 응답 계약

| 상태 | HTTP | 본문 |
|---|---|---|
| 활성 job 취소 성공 | 200 | `{ "cancelled": true, "job_id": "..." }` |
| 이미 완료/취소된 job | 200 | `{ "cancelled": true, "job_id": "..." }` (idempotent) |
| 미등록 / evict 됨 | 404 | `{ "error": "job not found or already evicted" }` |

### 통합 테스트 요구

`crates/secall-core/src/jobs/executor.rs` 의 `#[cfg(test)] mod tests` 에 다음 시나리오 2건 추가:
1. **cancel_marks_job_as_interrupted_and_emits_failed_event** — 충분히 긴 가짜 어댑터 spawn → cancel 호출 → 1초 이내에 registry.get(id).status == Interrupted, SSE 채널에서 Failed 이벤트 수신 검증
2. **cancel_unknown_job_returns_false** — 미등록 id 로 cancel → false 반환

각 테스트는 기존 `make_executor` helper 패턴 따라 작성.

## Dependencies

- 외부 crate: `tokio-util` (이미 workspace dep). default features 로 `CancellationToken` 사용 가능.
- 내부 task: 없음

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib jobs::executor::tests::cancel
cargo test -p secall-core --lib jobs::executor::tests
```

## Risks

- **`BroadcastSink::new` 시그니처 변경**: 호출처는 executor.rs 1곳. `cargo check` 가 컴파일러로 확인.
- **`registry.register` 시그니처 변경**: 호출처 동일 1곳. 동일 보장.
- **trait default 메서드 추가는 후방 호환**: 기존 ProgressSink 구현체(NoopSink, BroadcastSink, 외부 추가 구현이 있다면)는 default false 사용. 본 task 에서 NoopSink 도 명시 구현 추가 권장.
- **cancel 후 race**: 어댑터가 token cancel 직전에 마지막 phase 끝내고 Ok 반환할 수 있음. 이 경우 was_cancelled 판정으로 status 를 Interrupted 로 강제 → 사용자 관점 일관성 유지.
- **CancellationToken 이중 cancel**: tokio-util `CancellationToken::cancel` 은 idempotent. 안전.
- **evict 타이밍**: 기존 5분 보존 정책 유지. cancel 직후 곧바로 evict 안 함 → SSE 구독자가 final event 받을 시간 확보.
- **트랜잭션 도중 select! 강제 drop**: 어댑터가 DB 트랜잭션 중간이면 future drop 시 트랜잭션 rollback. 디벨로퍼는 어댑터(Task 01)에서 트랜잭션 단위는 끊지 않도록 안전 지점 위치 신중히 결정 — 본 task 는 인프라만 제공.

## Scope boundary

수정 금지:
- `crates/secall/src/commands/{sync,ingest,wiki}.rs` 의 `run_with_progress` 본문 — Task 01 영역. 단 NoopSink 의 trait 구현 추가는 본 task.
- `web/` 전체 — Task 02 영역
- `README*`, `.github/` — Task 03 영역
- `crates/secall-core/src/store/`, `crates/secall-core/src/mcp/server.rs` — 무관
- 기존 `ProgressEvent` variant — 추가/수정 없음 (`Failed.partial_result` 활용)
