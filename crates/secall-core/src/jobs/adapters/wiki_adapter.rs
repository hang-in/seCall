//! P33 Task 02 — wiki update 어댑터 시그니처.
//!
//! 실제 클로저 인스턴스는 `secall::commands::serve`에서 다음 패턴으로 조립된다:
//!
//! ```ignore
//! let wiki_update_fn: WikiUpdateAdapterFn = Box::new(|val, sink| {
//!     Box::pin(async move {
//!         let args: secall::commands::wiki::WikiUpdateArgs = serde_json::from_value(val)?;
//!         let outcome = secall::commands::wiki::run_with_progress(args, &sink).await?;
//!         Ok(serde_json::to_value(outcome)?)
//!     })
//! });
//! ```
//!
//! ## Args 사양 (`WikiUpdateArgs`)
//! - `model: Option<String>` — LLM 모델명 override
//! - `backend: Option<String>` — `claude` | `codex` | `haiku` | `ollama` | `lmstudio`
//! - `since: Option<String>` — `YYYY-MM-DD` 이후 신규 세션만
//! - `session: Option<String>` — 단일 세션 id로 incremental 갱신
//! - `dry_run: bool` — 프롬프트만 출력
//! - `review: bool` — 생성 후 review 모델로 검수
//! - `review_model: Option<String>`
//!
//! ## Outcome 사양 (`WikiOutcome`)
//! - `backend: String` — 실제 사용된 백엔드명
//! - `target: String` — 대상 (single session id 또는 batch since date)
//! - `pages_written: usize`
//!
//! ## Phase 흐름
//! `prompt_build → llm_call → lint → merge_and_write`
//!
//! 현재는 phase 경계만 외부에서 보고하고 내부는 기존 `run_update`에 위임.
//! 정밀한 phase 보고(예: 페이지별 progress)는 후속 task에서 `run_update` 분해 시 가능.

use super::AdapterFn;

/// wiki update 어댑터 클로저 타입. `AdapterFn`의 의미적 alias.
pub type WikiUpdateAdapterFn = AdapterFn;
