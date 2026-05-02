//! P33 Task 02 — ingest 어댑터 시그니처.
//!
//! 실제 클로저 인스턴스는 `secall::commands::serve`에서 다음 패턴으로 조립된다:
//!
//! ```ignore
//! let ingest_fn: IngestAdapterFn = Box::new(|val, sink| {
//!     Box::pin(async move {
//!         let args: secall::commands::ingest::IngestArgs = serde_json::from_value(val)?;
//!         let outcome = secall::commands::ingest::run_with_progress(args, &sink).await?;
//!         Ok(serde_json::to_value(outcome)?)
//!     })
//! });
//! ```
//!
//! ## Args 사양 (`IngestArgs`)
//! - `path: Option<String>` — 단일 파일 명시 ingest
//! - `auto: bool` — Claude Code/Codex/Gemini 자동 감지
//! - `cwd: Option<PathBuf>` — 프로젝트 디렉토리 필터
//! - `min_turns: usize` — 최소 turn 수 필터 (기본 0)
//! - `force: bool` — 중복 세션 재인제스트
//! - `no_semantic: bool` — semantic graph 추출 skip
//! - `auto_graph: bool` — ingest 후 graph 증분 활성 (기본 false, CLI는 명시 필요)
//!
//! ## Outcome 사양 (`IngestOutcome`)
//! - `ingested: usize`
//! - `skipped: usize`
//! - `errors: usize`
//! - `skipped_min_turns: usize`
//! - `hook_failures: usize`
//! - `new_session_ids: Vec<String>` — 그래프 자동 증분에 사용
//! - `graph_nodes_added`, `graph_edges_added: Option<usize>`
//!
//! ## Phase 흐름
//! `detect → parse_and_insert`
//!
//! 세분화는 의도적으로 회피 (P33 Task 02 사양). 큰 데이터 셋에서 SSE 트래픽 폭증 방지.

use super::AdapterFn;

/// ingest 어댑터 클로저 타입. `AdapterFn`의 의미적 alias.
pub type IngestAdapterFn = AdapterFn;
