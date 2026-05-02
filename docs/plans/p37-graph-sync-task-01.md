---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p37-graph-sync
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — CLI `graph rebuild` 명령 + GraphRebuildArgs/Outcome + `run_with_progress`

## Changed files

수정:
- `crates/secall/src/main.rs:347-359` 인접 — `GraphCommand` (또는 동등 enum) 에 `Rebuild { ... }` variant 추가. 기존 `Build` / `Stats` / `Export` 와 같은 깊이.
- `crates/secall/src/commands/graph.rs` — 새 핸들러 추가:
  - `GraphRebuildArgs` 구조체 (Task 00 의 `GraphRebuildFilter` 와 동등 + serde derive 로 REST DTO 호환)
  - `GraphRebuildOutcome` 구조체 (`processed`, `succeeded`, `failed`, `skipped`, `edges_added`)
  - `pub async fn run_rebuild(args, sink: &dyn ProgressSink) -> Result<GraphRebuildOutcome>`
  - CLI wrapper `pub async fn run_rebuild_cli(args) -> Result<()>` — `NoopSink` wrapper, P36 `run_update` 패턴
- `crates/secall/src/commands/ingest.rs:606-700` — 시맨틱 추출 핵심 루프 본문을 그대로 두되, `run_rebuild` 가 호출할 수 있도록 단일 세션 처리 함수 (`extract_one_session_semantic` 같은 이름) 로 추출. **시그니처만 분리**, 동작 변경 없음. ingest 측 호출처도 동일 helper 사용 → 중복 로직 제거 + 일관성.
- `crates/secall-core/src/store/session_repo.rs` (Task 01 에서 추가된 helper 만 사용 — 본 task 는 무수정).

신규: 없음 (기존 graph.rs 에 핸들러 추가)

## Change description

### 1. CLI 서브커맨드 정의 (main.rs)

기존 `secall graph build/stats/export` 옆에 `Rebuild` variant 추가. clap 인자:
- `--since YYYY-MM-DD` (`Option<String>`)
- `--session ID` (`Option<String>`)
- `--all` (bool flag)
- `--retry-failed` (bool flag)

핸들러는 `commands::graph::run_rebuild_cli(args).await?` 호출.

### 2. `GraphRebuildArgs` / `GraphRebuildOutcome` 구조체 계약

```rust
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct GraphRebuildArgs {
    pub since: Option<String>,
    pub session: Option<String>,
    pub all: bool,
    pub retry_failed: bool,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct GraphRebuildOutcome {
    pub processed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub edges_added: usize,
}
```

`Args` → `GraphRebuildFilter` 변환은 `From` impl 또는 Task 02 함수 내부에서 직접 인스턴스화. REST 측 (Task 02) 도 같은 구조체를 직렬화해서 사용.

### 3. `run_rebuild(args, sink)` 흐름 (P33 + P36 패턴)

1. `db.list_sessions_for_graph_rebuild(filter)` 로 처리 대상 ID 목록 획득
2. 빈 목록이면 `sink.message("처리할 세션 없음")` + `Ok(default outcome)` 반환
3. `let total = ids.len()`
4. for (i, id) in ids.iter().enumerate():
   - **안전 지점 (P36 패턴)**: `if sink.is_cancelled()` → `sink.message("취소 요청 — N/M 처리 후 종료")` + `Ok(partial outcome)` early return
   - `sink.progress((i as f32) / (total as f32))`
   - vault 에서 마크다운 읽기 (ingest.rs:660-690 의 로직 재사용)
   - `extract_one_session_semantic(db, &config, &fm, &body)` 호출 (ingest.rs 에서 추출한 단일 세션 helper)
   - 결과:
     - `Ok(edges)` → `outcome.succeeded += 1; outcome.edges_added += edges; db.update_semantic_extracted_at(id, now)?`
     - `Err(_)` → `outcome.failed += 1; semantic_extracted_at 갱신 안 함` (다음 retry-failed 대상이 됨)
     - vault 파일 없음 / 파싱 실패 → `outcome.skipped += 1`
   - `outcome.processed += 1`
5. 마지막에 `sink.message("완료: succeeded=N, failed=M, edges=K")` + `Ok(outcome)`

### 4. 단일 세션 추출 helper 분리 (ingest.rs)

현재 `ingest::run_internal` (또는 동등) 안의 시맨틱 처리 루프 본문 (line 658-700 근처) 을 다음 시그니처로 추출:

```rust
async fn extract_one_session_semantic(
    db: &Database,
    config: &Config,
    session_id: &str,
) -> ExtractOneResult; // enum: Extracted(usize) | Skipped(reason) | Failed(anyhow::Error)
```

기존 ingest 측 루프는 이 helper 를 호출하도록 단순화. 동작 변경 없음 (회귀 테스트 그대로 통과).

### 5. CLI wrapper (`run_rebuild_cli`)

`NoopSink` 사용. P36 의 `run_update`(wiki) 패턴 그대로:

```rust
pub async fn run_rebuild_cli(args: GraphRebuildArgs) -> Result<()> {
    let outcome = run_rebuild(args, &NoopSink).await?;
    eprintln!(
        "Graph rebuild complete: processed={}, succeeded={}, failed={}, skipped={}, edges_added={}",
        outcome.processed, outcome.succeeded, outcome.failed, outcome.skipped, outcome.edges_added,
    );
    Ok(())
}
```

### 6. 통합 테스트 (commands/graph.rs 또는 tests/)

- `test_run_rebuild_retry_failed_only_processes_null_sessions` — 가짜 DB 에 3 세션 (2 NULL, 1 timestamp) → `retry_failed=true` 로 `run_rebuild` 호출 → outcome.processed == 2 검증
- `test_run_rebuild_session_filter_processes_one` — single ID 지정 → outcome.processed == 1
- 시맨틱 추출 자체는 외부 의존성 (LLM) 큼 → 테스트는 fake/mock 또는 `extract_and_store` 가 NoopBackend 일 때 0 edges 반환하는 분기 활용

## Dependencies

- 외부 crate: 없음
- 내부 task: **Task 00 완료 필수** — `GraphRebuildFilter`, `update_semantic_extracted_at`, `list_sessions_for_graph_rebuild` API

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall --lib commands::graph::tests::test_run_rebuild
cargo test --all  # 기존 ingest 회귀 (단일 세션 helper 추출 영향 검증)

# 라이브 (선택, vault + DB + Ollama/Gemini 필요):
# secall graph rebuild --retry-failed
# → 처리 대상 출력 + progress + 완료 통계
```

## Risks

- **ingest helper 추출 회귀**: 시맨틱 처리 루프를 `extract_one_session_semantic` 으로 추출하면 ingest 의 기존 동작이 바뀌지 않아야 함. 회귀 테스트 (P26 / 기존 ingest 테스트) 통과 확인.
- **embedding 모델 unload (ingest.rs:614-632)**: 시맨틱 추출 전에 임베딩 모델 unload 하는 로직이 있음. 본 task 의 `run_rebuild` 도 동일 처리 필요 → helper 함수에 포함하거나 `run_rebuild` 진입 시점에 한 번 실행.
- **partial cancel 시 succeeded/edges 보존**: cancel 분기에서 outcome 그대로 `Ok(...)` 반환 → P36 executor 가 partial_result 보존.
- **빈 목록 처리**: 필터 결과 0 세션이면 정상 종료. 사용자에게 "처리할 세션 없음" 안내.
- **DB lock contention**: 단일 세션마다 update_semantic_extracted_at — 짧은 트랜잭션. P33 단일 큐 정책으로 sync/ingest 와 동시 실행 안 됨 → 충돌 없음.
- **vault 파일 누락 세션**: skipped 카운트로 처리, semantic_extracted_at 갱신 안 함 (다음 retry-failed 대상). 사용자 결정.

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/` — Task 00 영역 (단, helper 호출은 OK)
- `crates/secall-core/src/jobs/`, `crates/secall-core/src/mcp/` — Task 02 영역
- `crates/secall-core/src/graph/semantic/`, `crates/secall-core/src/graph/extract.rs` — 시맨틱 추출 로직 자체 (Non-goals)
- `crates/secall/src/commands/{sync,wiki,mod}.rs` — 무관
- `web/`, `README*`, `.github/` — Task 03/04 영역
