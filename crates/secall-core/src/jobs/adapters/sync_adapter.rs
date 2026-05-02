//! P33 Task 02 — sync 어댑터 시그니처.
//!
//! 실제 클로저 인스턴스는 `secall::commands::serve`에서 다음 패턴으로 조립된다:
//!
//! ```ignore
//! let sync_fn: SyncAdapterFn = Box::new(|val, sink| {
//!     Box::pin(async move {
//!         let args: secall::commands::sync::SyncArgs = serde_json::from_value(val)?;
//!         let outcome = secall::commands::sync::run_with_progress(args, &sink).await?;
//!         Ok(serde_json::to_value(outcome)?)
//!     })
//! });
//! ```
//!
//! ## Args 사양 (`SyncArgs`)
//! - `local_only: bool` — git pull/push 생략
//! - `dry_run: bool` — 변경 미적용
//! - `no_wiki: bool` — wiki update phase skip
//! - `no_semantic: bool` — semantic graph 추출 skip
//! - `no_graph: bool` — graph 자동 증분 phase skip (기본 false)
//!
//! ## Outcome 사양 (`SyncOutcome`)
//! - `pulled: Option<usize>`
//! - `reindexed: usize`
//! - `ingested: usize`
//! - `wiki_updated: Option<usize>`
//! - `pushed: Option<String>`
//! - `partial_failure: Option<String>` — push 실패 시에도 ingest 결과 보존
//! - `graph_nodes_added`, `graph_edges_added: Option<usize>`
//!
//! ## Phase 흐름
//! `init → pull → reindex → ingest → wiki_update → graph → push`
//!
//! 각 phase 경계에서 `BroadcastSink::phase_start/phase_complete`가 호출되어
//! SSE 구독자에게 진행 상태가 스트리밍된다.

use super::AdapterFn;

/// sync 어댑터 클로저 타입. `AdapterFn`의 의미적 alias.
pub type SyncAdapterFn = AdapterFn;
