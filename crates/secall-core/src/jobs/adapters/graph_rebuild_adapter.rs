//! P37 Task 02 — graph rebuild 어댑터 시그니처.
//!
//! 실제 클로저 인스턴스는 `secall::commands::serve`에서 다음 패턴으로 조립된다:
//!
//! ```ignore
//! let graph_rebuild_fn: GraphRebuildAdapterFn = Box::new(|val, sink| {
//!     Box::pin(async move {
//!         let args: secall::commands::graph::GraphRebuildArgs = serde_json::from_value(val)?;
//!         let outcome = secall::commands::graph::run_rebuild(args, &sink).await?;
//!         Ok(serde_json::to_value(outcome)?)
//!     })
//! });
//! ```
//!
//! ## Args 사양 (`GraphRebuildArgs`)
//! - `since: Option<String>` — `YYYY-MM-DD` 이후 세션만 (`semantic_extracted_at IS NULL` 또는 `< since`)
//! - `session: Option<String>` — 단일 세션 id 만 처리
//! - `all: bool` — 모든 세션 강제 재처리
//! - `retry_failed: bool` — `semantic_extracted_at IS NULL` 인 세션만 (실패/미처리 재시도)
//!
//! 우선순위: `session` > `all` > `retry_failed` > `since`. 모두 비활성이면 빈 결과.
//!
//! ## Outcome 사양 (`GraphRebuildOutcome`)
//! - `processed: usize`
//! - `succeeded: usize`
//! - `failed: usize`
//! - `skipped: usize`
//! - `edges_added: usize`
//!
//! ## Phase 흐름
//! 단일 phase. 매 세션 시작 지점에서 `is_cancelled()` 폴링 후 부분 outcome 으로
//! early return — P36 cancel 패턴이 그대로 적용된다. partial outcome 은 executor 의
//! `was_cancelled` 게이팅이 `result_json` / SSE `Failed.partial_result` 에 보존한다.

use super::AdapterFn;

/// graph rebuild 어댑터 클로저 타입. `AdapterFn`의 의미적 alias.
pub type GraphRebuildAdapterFn = AdapterFn;
