//! P33 Task 00 — Jobs 테이블 repository.
//!
//! 메모리(JobRegistry, Task 01)는 진행 중 상태만 보유하고, 완료/실패/중단 시점에
//! `complete_job`으로 영구 기록한다. 시작 시 `cleanup_old_jobs`로 7일 이상된 row 삭제.

use crate::error::Result;
use crate::store::db::Database;

/// jobs 테이블 row를 그대로 매핑한 구조체.
///
/// `result`/`metadata`는 SQLite에 TEXT로 저장된 JSON. 직렬화 실패 시 None으로 폴백.
#[derive(Debug, Clone, serde::Serialize)]
pub struct JobRow {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

impl Database {
    /// Job 시작 시 INSERT. status는 `started`로 기록.
    pub fn insert_job(
        &self,
        id: &str,
        kind: &str,
        metadata: Option<&serde_json::Value>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let metadata_json = metadata.and_then(|v| serde_json::to_string(v).ok());
        self.conn().execute(
            "INSERT INTO jobs(id, kind, status, started_at, metadata)
             VALUES (?1, ?2, 'started', ?3, ?4)",
            rusqlite::params![id, kind, now, metadata_json],
        )?;
        Ok(())
    }

    /// 완료/실패/중단 시 상태 갱신 + completed_at 기록. running 갱신은 메모리에만.
    pub fn complete_job(
        &self,
        id: &str,
        status: &str,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let result_json = result.and_then(|v| serde_json::to_string(v).ok());
        self.conn().execute(
            "UPDATE jobs
             SET status = ?1, completed_at = ?2, result = ?3, error = ?4
             WHERE id = ?5",
            rusqlite::params![status, now, result_json, error, id],
        )?;
        Ok(())
    }

    /// 단일 Job 조회. 없으면 Ok(None).
    pub fn get_job(&self, id: &str) -> Result<Option<JobRow>> {
        match self.conn().query_row(
            "SELECT id, kind, status, started_at, completed_at, error, result, metadata
             FROM jobs WHERE id = ?1",
            rusqlite::params![id],
            row_to_jobrow,
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 최근 N개 jobs (started_at DESC).
    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<JobRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, kind, status, started_at, completed_at, error, result, metadata
             FROM jobs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![limit as i64], row_to_jobrow)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 7일 이상된 완료/실패/중단 jobs 삭제. 반환값: 삭제된 row 수.
    pub fn cleanup_old_jobs(&self) -> Result<usize> {
        let n = self.conn().execute(
            "DELETE FROM jobs WHERE completed_at IS NOT NULL AND completed_at < datetime('now', '-7 days')",
            [],
        )?;
        Ok(n)
    }
}

fn row_to_jobrow(row: &rusqlite::Row) -> rusqlite::Result<JobRow> {
    let result_text: Option<String> = row.get(6)?;
    let metadata_text: Option<String> = row.get(7)?;
    Ok(JobRow {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        started_at: row.get(3)?,
        completed_at: row.get(4)?,
        error: row.get(5)?,
        result: result_text
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        metadata: metadata_text
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    })
}
