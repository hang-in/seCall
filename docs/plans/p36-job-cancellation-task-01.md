---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p36-job-cancellation
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — Adapter 통합 (sync/ingest/wiki) — 안전 지점에 cancel check

## Changed files

수정:
- `crates/secall/src/commands/sync.rs:67` — `run_with_progress` 함수 본문의 phase 사이마다 `sink.is_cancelled()` 폴링 + cancel 시 지금까지의 누적 `SyncOutcome` 으로 early return.
- `crates/secall/src/commands/ingest.rs:207` — `run_with_progress` 함수 본문의 file processing 루프 시작 지점마다 cancel 폴링 + 부분 누적 `IngestOutcome` 반환.
- `crates/secall/src/commands/wiki.rs:42` — `run_with_progress` 함수 본문의 session/page 루프 시작 지점 (특히 LLM 호출 직전) 마다 cancel 폴링 + 부분 누적 `WikiOutcome` 반환.

신규: 없음

## Change description

### 공통 패턴

각 `run_with_progress` 본문에 다음 폴링 패턴 삽입:
- 안전 지점에서 `sink.is_cancelled()` 호출
- true 면 `sink.message("취소 요청 — 부분 결과로 종료합니다")` 안내 후 누적 outcome 으로 `Ok(...)` 반환 (Err 가 아닌 Ok — JobExecutor 가 status 를 Interrupted 로 마킹)

### 안전 지점 정의 (어댑터별)

| 파일 | 안전 지점 위치 | 이유 |
|---|---|---|
| `sync.rs` | 각 phase(`pull` / `reindex` / `ingest` / `wiki_update` / `push`) 시작 직전 | phase 내부의 외부 명령(`git pull` 등) 도중 cancel 은 본 phase 외 |
| `ingest.rs` | file 단위 루프 시작 지점 (`for file in files`) | 단일 file 처리 도중 (graph extract 트랜잭션 등) 는 끊지 않음 |
| `wiki.rs` | session/page 단위 루프 시작 지점 + LLM 호출 직전 | LLM 호출은 비싸므로 미리 차단 |

**금지 위치**: DB 트랜잭션 내부, 외부 명령 spawn 직후 단일 함수 내부, `process_file()` 같은 단일 단위 실행 중간.

### 부분 결과 보존 계약

각 outcome 구조체(`SyncOutcome`, `IngestOutcome`, `WikiOutcome`)는 누적형:
- `IngestOutcome.ingested` — 취소 시점까지 처리된 카운트
- `WikiOutcome.pages_written` — 작성된 페이지 카운트
- `SyncOutcome.{pulled, reindexed, wiki_updated, pushed}` — 완료된 phase 의 boolean/숫자

cancel 발생 시 outcome 의 미완료 phase 필드는 `None` 또는 0 으로 둔다 (이미 default 값). 사용자(웹 UI)는 outcome 을 보고 어디까지 됐는지 파악 가능.

### 진행률 보고

각 루프 안에서 `sink.progress(i / total)` 호출 — cancel 응답 지연을 줄이기 위해 매 iteration 마다 호출 권장.

### 위치 검증 절차

디벨로퍼는 다음 순서로 진행:
1. 각 `run_with_progress` 함수 본문 읽고 phase / loop 위치 식별
2. 위 표의 안전 지점에 폴링 코드 삽입
3. 헬퍼 함수로 분리되어 sink reference 가 닿지 않는 위치는 helper 시그니처에 sink 추가
4. cancel check 가 트랜잭션 / 외부 명령 도중에 들어가지 않도록 신중히 위치 결정

## Dependencies

- 외부 crate: 없음
- 내부 task: **Task 00 완료 필수** — `ProgressSink::is_cancelled` 메서드가 trait 에 있어야 함

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib jobs::executor::tests::cancel  # Task 00 회귀
cargo test --all                                                 # sync/ingest/wiki 기존 테스트 회귀

# 라이브 (선택, 서버 + 충분한 input 필요):
# 1) secall serve --bind 127.0.0.1:8080 &
# 2) curl -X POST http://127.0.0.1:8080/api/commands/ingest -d '{"path":"...많은 파일..."}'
# 3) 즉시 curl -X POST http://127.0.0.1:8080/api/jobs/<id>/cancel
# 4) curl http://127.0.0.1:8080/api/jobs/<id> → status: "interrupted", result.ingested 부분 카운트
```

## Risks

- **cancel check 누락**: phase / loop 내부에 긴 sub-loop 가 있으면 cancel 응답 지연. 본 task 는 outer 경계만 → 5초 응답 목표 만족.
- **부분 결과의 의미적 모호**: `outcome.pulled = Some(true)` 인데 reindex 안 된 경우 사용자 혼란. `sink.message("취소 요청 — ... 후 종료")` 로 보완.
- **DB 트랜잭션 도중 select! 강제 drop 위험**: Task 00 의 select! 가 future 를 drop 시킬 수 있음. 어댑터에서 트랜잭션 단위 가 폴링 지점과 겹치지 않도록 주의 — 본 task 디벨로퍼의 핵심 책임.
- **LLM 비용**: wiki review 가 두 번 LLM 호출. 두 호출 사이에도 폴링 시 비용 절감.
- **테스트 어려움**: 실제 sync/ingest/wiki 는 외부 의존성(git, FS, LLM) 큼 → 통합 테스트는 fake adapter 로 Task 00 회귀 케이스로 충분. 본 task 는 cancel check 삽입이므로 cargo check + clippy + 기존 회귀로 검증.

## Scope boundary

수정 금지:
- `crates/secall-core/src/jobs/` 전체 — Task 00 영역
- `crates/secall-core/src/mcp/`, `crates/secall-core/src/store/` — 무관
- `crates/secall/src/commands/mod.rs` (NoopSink) — Task 00 에서 처리
- `crates/secall/src/main.rs` — 무관
- `web/` 전체 — Task 02 영역
- `README*`, `.github/` — Task 03 영역
- 기존 outcome 구조체 필드 — 추가/수정 없음 (이미 충분)
