use crate::search::vector::VectorRow;
use crate::store::db::Database;

pub trait VectorRepo {
    fn init_vector_table(&self) -> anyhow::Result<()>;
    fn insert_vector(
        &self,
        embedding: &[f32],
        session_id: &str,
        turn_index: u32,
        chunk_seq: u32,
        model: &str,
    ) -> anyhow::Result<i64>;
    fn search_vectors(
        &self,
        query_embedding: &[f32],
        limit: usize,
        session_ids: Option<&[String]>,
    ) -> crate::error::Result<Vec<VectorRow>>;
    /// rowid로 turn_vectors의 (session_id, turn_index, chunk_seq) 조회.
    /// ANN 검색 결과를 DB 메타데이터와 연결할 때 사용.
    fn get_vector_meta(&self, rowid: i64) -> anyhow::Result<(String, u32, u32)>;
}

// VectorRepo impl for Database — vector table management + search
impl VectorRepo for Database {
    fn init_vector_table(&self) -> anyhow::Result<()> {
        self.conn().execute_batch(
            "
            CREATE TABLE IF NOT EXISTS turn_vectors (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  TEXT NOT NULL,
                turn_index  INTEGER NOT NULL,
                chunk_seq   INTEGER NOT NULL,
                model       TEXT NOT NULL,
                embedded_at TEXT NOT NULL,
                embedding   BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vectors_session ON turn_vectors(session_id);
        ",
        )?;
        Ok(())
    }

    fn insert_vector(
        &self,
        embedding: &[f32],
        session_id: &str,
        turn_index: u32,
        chunk_seq: u32,
        model: &str,
    ) -> anyhow::Result<i64> {
        if embedding.is_empty() {
            anyhow::bail!("empty embedding for session={session_id} turn={turn_index}");
        }

        // 기존 데이터와 차원 일치 확인 (첫 삽입 시 건너뜀)
        let existing_dim: Option<usize> = self
            .conn()
            .query_row(
                "SELECT LENGTH(embedding) FROM turn_vectors LIMIT 1",
                [],
                |row| row.get::<_, i64>(0).map(|n| n as usize / 4),
            )
            .ok();

        if let Some(dim) = existing_dim {
            if embedding.len() != dim {
                anyhow::bail!(
                    "embedding dimension mismatch: expected {dim}, got {} (session={session_id})",
                    embedding.len()
                );
            }
        }

        let bytes = floats_to_bytes(embedding);
        self.conn().execute(
            "INSERT INTO turn_vectors(session_id, turn_index, chunk_seq, model, embedded_at, embedding)
             VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
            rusqlite::params![session_id, turn_index as i64, chunk_seq as i64, model, bytes],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    fn search_vectors(
        &self,
        query_embedding: &[f32],
        limit: usize,
        session_ids: Option<&[String]>,
    ) -> crate::error::Result<Vec<VectorRow>> {
        let row_mapper = |row: &rusqlite::Row<'_>| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i64>(2)? as u32,
                row.get::<_, i64>(3)? as u32,
                row.get(4)?,
            ))
        };

        let rows: Vec<(i64, String, u32, u32, Vec<u8>)> = if let Some(ids) = session_ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "SELECT id, session_id, turn_index, chunk_seq, embedding \
                 FROM turn_vectors WHERE session_id IN ({})",
                placeholders.join(",")
            );
            let mut stmt = self.conn().prepare(&sql)?;
            let collected: Vec<_> = stmt
                .query_map(rusqlite::params_from_iter(ids.iter()), row_mapper)?
                .filter_map(|r| r.ok())
                .collect();
            collected
        } else {
            let mut stmt = self.conn().prepare(
                "SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors",
            )?;
            let collected: Vec<_> = stmt
                .query_map([], row_mapper)?
                .filter_map(|r| r.ok())
                .collect();
            collected
        };

        let mut scored: Vec<(f32, VectorRow)> = rows
            .into_iter()
            .map(|(id, session_id, turn_index, chunk_seq, bytes)| {
                let embedding = bytes_to_floats(&bytes);
                let distance = cosine_distance(query_embedding, &embedding);
                (
                    distance,
                    VectorRow {
                        rowid: id,
                        distance,
                        session_id,
                        turn_index,
                        chunk_seq,
                    },
                )
            })
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        Ok(scored.into_iter().map(|(_, row)| row).collect())
    }

    fn get_vector_meta(&self, rowid: i64) -> anyhow::Result<(String, u32, u32)> {
        self.conn()
            .query_row(
                "SELECT session_id, turn_index, chunk_seq FROM turn_vectors WHERE id = ?1",
                [rowid],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)? as u32,
                        row.get::<_, i64>(2)? as u32,
                    ))
                },
            )
            .map_err(Into::into)
    }
}

pub(crate) fn floats_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes
}

pub(crate) fn bytes_to_floats(bytes: &[u8]) -> Vec<f32> {
    if bytes.len() % 4 != 0 {
        tracing::warn!(
            blob_len = bytes.len(),
            "corrupt vector BLOB (not multiple of 4 bytes)"
        );
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

pub(crate) fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - (dot / (norm_a * norm_b))
}

// ─── Additional Database methods (vector domain) ─────────────────────────────

use crate::error::Result;

impl Database {
    pub fn has_embeddings(&self) -> Result<bool> {
        let exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Ok(false);
        }
        let count: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0))?;
        Ok(count > 0)
    }

    /// turn_vectors 테이블의 총 벡터 수. ANN stale 감지에 사용.
    pub fn count_vectors(&self) -> Result<usize> {
        let exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Ok(0);
        }
        let count: i64 = self
            .conn()
            .query_row("SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Sessions with at least one turn missing a vector row.
    ///
    /// Anti-joins `turns` against `turn_vectors` on `(session_id, turn_index)`,
    /// so this catches both fully-unembedded sessions (zero-vec) and partially
    /// embedded sessions (some turns committed, others missing — e.g. after
    /// transient embedder failures).
    ///
    /// Sessions with zero rows in `turns` are not returned (nothing to embed).
    pub fn find_sessions_without_vectors(&self) -> Result<Vec<String>> {
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;

        let query = if table_exists == 0 {
            "SELECT DISTINCT session_id FROM turns"
        } else {
            "SELECT DISTINCT session_id FROM turns AS t \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM turn_vectors AS v \
                 WHERE v.session_id = t.session_id AND v.turn_index = t.turn_index \
             )"
        };

        let mut stmt = self.conn().prepare(query)?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Existing `(turn_index, chunk_seq)` pairs already in `turn_vectors` for
    /// the given session. Used by `index_session` to skip already-embedded
    /// chunks (turn-incremental healing).
    pub fn get_session_chunk_keys(
        &self,
        session_id: &str,
    ) -> Result<std::collections::HashSet<(u32, u32)>> {
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(std::collections::HashSet::new());
        }
        let mut stmt = self
            .conn()
            .prepare("SELECT turn_index, chunk_seq FROM turn_vectors WHERE session_id = ?1")?;
        let rows = stmt.query_map([session_id], |row| {
            Ok((row.get::<_, i64>(0)? as u32, row.get::<_, i64>(1)? as u32))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Vector rows whose session_id does not exist in sessions
    pub fn find_orphan_vectors(&self) -> Result<Vec<(i64, String)>> {
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;

        if table_exists == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn().prepare(
            "SELECT id, session_id FROM turn_vectors WHERE session_id NOT IN (SELECT id FROM sessions)",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::ingest::{AgentKind, Role, Session, TokenUsage, Turn};
    use crate::store::db::Database;
    use crate::store::{SessionRepo, VectorRepo};
    use chrono::TimeZone;

    fn make_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: None,
            cwd: None,
            git_branch: None,
            host: None,
            start_time: chrono::Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
            end_time: None,
            turns: vec![],
            total_tokens: TokenUsage::default(),
            session_type: "interactive".to_string(),
        }
    }

    fn make_turn(idx: u32) -> Turn {
        Turn {
            index: idx,
            role: Role::User,
            timestamp: None,
            content: format!("turn {idx} content"),
            actions: vec![],
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }
    }

    /// 한 세션이 일부 turn에만 vector를 가진 경우, anti-join 기반 detection이
    /// 그 세션을 healing 대상으로 잡아야 한다 (partial commit 잔여분 healing).
    #[test]
    fn test_find_sessions_without_vectors_detects_partial() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        db.insert_session(&make_session("partial")).unwrap();
        db.insert_turn("partial", &make_turn(0)).unwrap();
        db.insert_turn("partial", &make_turn(1)).unwrap();
        db.insert_turn("partial", &make_turn(2)).unwrap();

        // turn 0, 1만 임베딩됨 — turn 2 누락
        db.insert_vector(&[1.0_f32, 0.0, 0.0], "partial", 0, 0, "test")
            .unwrap();
        db.insert_vector(&[0.0_f32, 1.0, 0.0], "partial", 1, 0, "test")
            .unwrap();

        let sessions = db.find_sessions_without_vectors().unwrap();
        assert!(
            sessions.contains(&"partial".to_string()),
            "partial session must be returned, got {:?}",
            sessions
        );
    }

    /// 모든 turn에 vector가 있는 세션은 healing 대상에서 제외.
    #[test]
    fn test_find_sessions_without_vectors_excludes_complete() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        db.insert_session(&make_session("complete")).unwrap();
        db.insert_turn("complete", &make_turn(0)).unwrap();
        db.insert_turn("complete", &make_turn(1)).unwrap();

        db.insert_vector(&[1.0_f32, 0.0, 0.0], "complete", 0, 0, "test")
            .unwrap();
        db.insert_vector(&[0.0_f32, 1.0, 0.0], "complete", 1, 0, "test")
            .unwrap();

        let sessions = db.find_sessions_without_vectors().unwrap();
        assert!(
            !sessions.contains(&"complete".to_string()),
            "complete session must be excluded, got {:?}",
            sessions
        );
    }

    /// Vector가 전혀 없는 세션도 잡힌다 (zero-vec — 기존 동작 유지).
    #[test]
    fn test_find_sessions_without_vectors_detects_zero() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        db.insert_session(&make_session("zero")).unwrap();
        db.insert_turn("zero", &make_turn(0)).unwrap();

        let sessions = db.find_sessions_without_vectors().unwrap();
        assert!(sessions.contains(&"zero".to_string()));
    }

    /// `get_session_chunk_keys`는 해당 세션의 `(turn_index, chunk_seq)` 집합을
    /// 정확히 반환해 turn-incremental 호출자가 누락 chunk만 골라내도록 한다.
    #[test]
    fn test_get_session_chunk_keys_returns_existing_pairs() {
        let db = Database::open_memory().unwrap();
        db.init_vector_table().unwrap();
        db.insert_vector(&[1.0_f32, 0.0, 0.0], "A", 0, 0, "test")
            .unwrap();
        db.insert_vector(&[0.0_f32, 1.0, 0.0], "A", 0, 1, "test")
            .unwrap();
        db.insert_vector(&[0.0_f32, 0.0, 1.0], "A", 1, 0, "test")
            .unwrap();
        // Different session — must not leak in
        db.insert_vector(&[1.0_f32, 1.0, 0.0], "B", 0, 0, "test")
            .unwrap();

        let keys = db.get_session_chunk_keys("A").unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&(0, 0)));
        assert!(keys.contains(&(0, 1)));
        assert!(keys.contains(&(1, 0)));
        assert!(!keys.contains(&(1, 1)));

        let other = db.get_session_chunk_keys("B").unwrap();
        assert_eq!(other.len(), 1);
        assert!(other.contains(&(0, 0)));

        let empty = db.get_session_chunk_keys("missing").unwrap();
        assert!(empty.is_empty());
    }
}
