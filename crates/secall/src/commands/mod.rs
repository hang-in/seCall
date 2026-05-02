pub mod classify;
pub mod config;
pub mod embed;
pub mod get;
pub mod graph;
pub mod ingest;
pub mod init;
pub mod lint;
pub mod log;
pub mod mcp;
pub mod migrate;
pub mod model;
pub mod recall;
pub mod reindex;
pub mod serve;
pub mod status;
pub mod sync;
pub mod wiki;

/// CLI 경로에서 `run_with_progress`를 호출할 때 사용하는 비활성 sink.
///
/// 기존 CLI 출력은 `eprintln!` 그대로 유지되며, sink 메서드는 모두 no-op이다.
/// 이로써 P33 Task 02의 phase 분리 도입 후에도 CLI 회귀가 발생하지 않는다.
pub(crate) struct NoopSink;

#[async_trait::async_trait]
impl secall_core::jobs::ProgressSink for NoopSink {
    async fn phase_start(&self, _phase: &str) {}
    async fn message(&self, _text: &str) {}
    async fn progress(&self, _ratio: f32) {}
    async fn phase_complete(&self, _phase: &str, _result: Option<serde_json::Value>) {}
}
