//! P33 Task 01 — Job 타입 정의.
//!
//! `JobKind`/`JobStatus`/`JobState`는 메모리 상태 + REST 응답에 공통으로 사용된다.
//! `ProgressEvent`는 SSE로 broadcast되는 이벤트 페이로드.

use serde::{Deserialize, Serialize};

/// Job 종류. mutating 작업만 등록되며, read-only 조회는 Executor를 거치지 않는다.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Sync,
    Ingest,
    WikiUpdate,
}

impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobKind::Sync => "sync",
            JobKind::Ingest => "ingest",
            JobKind::WikiUpdate => "wiki_update",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sync" => Some(JobKind::Sync),
            "ingest" => Some(JobKind::Ingest),
            "wiki_update" => Some(JobKind::WikiUpdate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// 큐 진입 또는 spawn 직후. 단일 큐 lock 대기 중인 상태 포함.
    Started,
    /// 단일 큐 lock 획득 후 실제 phase 실행 중.
    Running,
    Completed,
    Failed,
    /// 서버 재시작 등으로 중단. (Task 04에서 보정 호출)
    Interrupted,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Started => "started",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Interrupted => "interrupted",
        }
    }
}

/// 메모리 상의 Job 상태. SSE 재접속 시 마지막 상태 fetch에 사용.
#[derive(Debug, Clone, Serialize)]
pub struct JobState {
    pub id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// 예: "pull", "reindex", "ingest", "push"
    pub current_phase: Option<String>,
    /// 0.0 ~ 1.0 (선택)
    pub progress: Option<f32>,
    /// 최근 로그 한 줄 (UI 표시용)
    pub message: Option<String>,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Job 실행 중 progress reporter가 broadcast하는 SSE 이벤트.
///
/// `tag = "type"`로 직렬화되며, snake_case 변종이 SSE event 데이터로 사용된다.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressEvent {
    PhaseStart {
        phase: String,
    },
    Message {
        text: String,
    },
    Progress {
        ratio: f32,
    },
    PhaseComplete {
        phase: String,
        result: Option<serde_json::Value>,
    },
    Done {
        result: serde_json::Value,
    },
    Failed {
        error: String,
        partial_result: Option<serde_json::Value>,
    },
}
