//! P33 Task 02 — Job → 명령 어댑터 indirection.
//!
//! `secall-core`는 `secall` crate에 역의존할 수 없으므로, 실제 명령 함수
//! (sync/ingest/wiki update)는 `Box<dyn Fn>` indirection으로 주입받는다.
//!
//! `secall::main`(또는 `serve`)에서 `CommandAdapters`를 생성해
//! `JobExecutor` 측에 전달하면, REST 핸들러(Task 04)가 Job 종류에 따라
//! 해당 클로저를 호출하여 spawn한다.
//!
//! args는 `serde_json::Value`로 erasure하여 secall-core가 `SyncArgs` 등
//! 구체 타입을 알 필요가 없도록 한다. 호출 측에서 `serde_json::from_value`로
//! 역직렬화하여 사용한다.
//!
//! 각 명령별 어댑터 모듈은 `sync_adapter` / `ingest_adapter` / `wiki_adapter`로
//! 분리되어 있다. 이들 모듈은 작업 지시서(P33 Task 02 Changed files)에 명시된
//! 분리 구조를 유지하면서, 동일한 `AdapterFn` 타입을 재export하고
//! `CommandAdapters` 필드의 의미를 문서화한다. 실제 클로저 인스턴스는
//! `secall::commands::serve`에서 `BroadcastSink` + `serde_json::from_value`로
//! 조립된다.

use std::future::Future;
use std::pin::Pin;

use crate::jobs::BroadcastSink;

pub mod ingest_adapter;
pub mod sync_adapter;
pub mod wiki_adapter;

pub use ingest_adapter::IngestAdapterFn;
pub use sync_adapter::SyncAdapterFn;
pub use wiki_adapter::WikiUpdateAdapterFn;

/// Job 어댑터에 전달되는 args의 타입 별칭. REST 핸들러가 dto → Value로 변환해 넘긴다.
pub type SyncArgsValue = serde_json::Value;
pub type IngestArgsValue = serde_json::Value;
pub type WikiUpdateArgsValue = serde_json::Value;

/// 단일 어댑터 클로저 시그니처.
///
/// `(args, sink) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>>`
///
/// 각 명령별 alias (`SyncAdapterFn`, `IngestAdapterFn`, `WikiUpdateAdapterFn`)는
/// 동일한 타입의 의미적 wrapper다 — 이름으로 어떤 명령용인지 명확히 하기 위함.
pub type AdapterFn = Box<
    dyn Fn(
            serde_json::Value,
            BroadcastSink,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<serde_json::Value>> + Send>>
        + Send
        + Sync,
>;

/// secall-core가 호출 가능한 명령 어댑터 묶음.
///
/// `secall::main`/`serve.rs`에서 생성해 REST 핸들러 또는 `JobExecutor`로 전달.
pub struct CommandAdapters {
    /// `secall::commands::sync::run_with_progress(SyncArgs, &BroadcastSink)`의 wrapper.
    /// 자세한 args/outcome 사양은 [`sync_adapter`] 참조.
    pub sync_fn: SyncAdapterFn,
    /// `secall::commands::ingest::run_with_progress(IngestArgs, &BroadcastSink)`의 wrapper.
    /// 자세한 사양은 [`ingest_adapter`] 참조.
    pub ingest_fn: IngestAdapterFn,
    /// `secall::commands::wiki::run_with_progress(WikiUpdateArgs, &BroadcastSink)`의 wrapper.
    /// 자세한 사양은 [`wiki_adapter`] 참조.
    pub wiki_update_fn: WikiUpdateAdapterFn,
}
