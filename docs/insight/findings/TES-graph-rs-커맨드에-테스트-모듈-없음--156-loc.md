# graph.rs 커맨드에 테스트 모듈 없음 (156 LOC)

- **Category**: test
- **Severity**: minor
- **Fix Difficulty**: guided
- **Status**: in_progress
- **File**: crates/secall/src/commands/graph.rs:28

## Description

graph.rs는 5개의 pub 함수(run_semantic, run_build, run_stats, run_export)를 포함하지만 테스트가 없습니다. 핵심 로직은 secall-core의 `build_graph`, `export_graph_json`, `extract_and_store`에 위임되어 해당 crate에서 테스트되지만, `run_semantic`의 임베딩 모델 언로드 로직(keep_alive: 0)이나 에러 카운팅 로직은 커버되지 않습니다.

**Evidence**: `let _ = secall_core::http_post_json(
    &unload_url,
    &serde_json::json!({"model": embed_model, "keep_alive": 0}),
).await;`

## Snippet

```
pub async fn run_semantic(delay_secs: u64, limit: Option<usize>) -> Result<()> { ... }
```
