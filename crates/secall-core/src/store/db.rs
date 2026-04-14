use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
#[cfg(test)]
use crate::error::SecallError;

use super::schema::{
    CREATE_CONFIG, CREATE_GRAPH_EDGES, CREATE_GRAPH_INDEXES, CREATE_GRAPH_NODES, CREATE_INDEXES,
    CREATE_INGEST_LOG, CREATE_QUERY_CACHE, CREATE_SESSIONS, CREATE_TURNS, CREATE_TURNS_FTS,
    CURRENT_SCHEMA_VERSION,
};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&self) -> Result<()> {
        // Ensure config table exists first
        self.conn.execute_batch(CREATE_CONFIG)?;

        let version: Option<u32> = self
            .conn
            .query_row(
                "SELECT value FROM config WHERE key = 'schema_version'",
                [],
                |row| {
                    let v: String = row.get(0)?;
                    Ok(v.parse::<u32>().unwrap_or(0))
                },
            )
            .ok();

        let current = version.unwrap_or(0);

        if current < 1 {
            self.apply_v1()?;
        }
        if current < 2 {
            // Column migrations for v2
            if !self.column_exists("sessions", "host")? {
                self.conn
                    .execute("ALTER TABLE sessions ADD COLUMN host TEXT", [])?;
            }
            if !self.column_exists("sessions", "summary")? {
                self.conn
                    .execute("ALTER TABLE sessions ADD COLUMN summary TEXT", [])?;
            }
        }
        if current < 3 {
            self.conn.execute_batch(CREATE_GRAPH_NODES)?;
            self.conn.execute_batch(CREATE_GRAPH_EDGES)?;
            self.conn.execute_batch(CREATE_GRAPH_INDEXES)?;
        }
        if current < 4 && !self.column_exists("sessions", "session_type")? {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN session_type TEXT DEFAULT 'interactive'",
                [],
            )?;
        }
        if current < CURRENT_SCHEMA_VERSION {
            self.conn.execute(
                "INSERT OR REPLACE INTO config(key, value) VALUES ('schema_version', ?1)",
                [CURRENT_SCHEMA_VERSION.to_string()],
            )?;
        }

        // Non-versioned additions: always apply (CREATE IF NOT EXISTS)
        self.conn.execute_batch(CREATE_QUERY_CACHE)?;

        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
            rusqlite::params![table, column],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn apply_v1(&self) -> Result<()> {
        self.conn.execute_batch(CREATE_SESSIONS)?;
        self.conn.execute_batch(CREATE_TURNS)?;
        self.conn.execute_batch(CREATE_TURNS_FTS)?;
        self.conn.execute_batch(CREATE_INGEST_LOG)?;
        self.conn.execute_batch(CREATE_INDEXES)?;
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Execute a closure within a SQLite transaction.
    /// Commits on Ok, rolls back on Err.
    pub fn with_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        self.conn.execute_batch("BEGIN")?;
        match f() {
            Ok(val) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(val)
            }
            Err(e) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DbStats> {
        let session_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        let turn_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))?;
        let vector_count: i64 = {
            let exists: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
                [],
                |r| r.get(0),
            )?;
            if exists > 0 {
                self.conn
                    .query_row("SELECT COUNT(*) FROM turn_vectors", [], |r| r.get(0))?
            } else {
                0
            }
        };

        let mut stmt = self.conn.prepare(
            "SELECT il.session_id, s.agent, il.timestamp
             FROM ingest_log il
             LEFT JOIN sessions s ON il.session_id = s.id
             WHERE il.action = 'ingest'
             ORDER BY il.id DESC LIMIT 5",
        )?;
        let recent_ingests = stmt
            .query_map([], |row| {
                let sid: String = row.get(0)?;
                let agent: Option<String> = row.get(1)?;
                let ts: String = row.get(2)?;
                Ok(IngestLogEntry {
                    session_id_prefix: sid[..sid.len().min(8)].to_string(),
                    agent: agent.unwrap_or_else(|| "unknown".to_string()),
                    timestamp: ts[..ts.len().min(10)].to_string(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(DbStats {
            session_count,
            turn_count,
            vector_count,
            recent_ingests,
        })
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> Result<u32> {
        let v: String = self.conn.query_row(
            "SELECT value FROM config WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )?;
        v.parse()
            .map_err(|e: std::num::ParseIntError| SecallError::Other(e.into()))
    }

    #[cfg(test)]
    pub fn table_exists(&self, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DbStats {
    pub session_count: i64,
    pub turn_count: i64,
    pub vector_count: i64,
    pub recent_ingests: Vec<IngestLogEntry>,
}

#[derive(Debug)]
pub struct IngestLogEntry {
    pub session_id_prefix: String,
    pub agent: String,
    pub timestamp: String,
}

#[derive(Debug)]
pub struct TurnRow {
    pub turn_index: u32,
    pub role: String,
    pub content: String,
}

/// 세션 메타데이터 (위키 생성용 경량 구조체)
#[derive(Debug)]
pub struct SessionMeta {
    pub id: String,
    pub agent: String,
    pub project: Option<String>,
    pub summary: Option<String>,
    pub start_time: String,
    pub turn_count: i64,
    pub tools_used: Option<String>,
    pub session_type: String,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{AgentKind, Role, Session, TokenUsage, Turn};
    use crate::store::SessionRepo;
    use chrono::TimeZone;

    fn make_test_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            agent: AgentKind::ClaudeCode,
            model: Some("claude-sonnet-4-6".to_string()),
            project: Some("test-project".to_string()),
            cwd: None,
            git_branch: None,
            host: None,
            start_time: chrono::Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap(),
            end_time: None,
            turns: vec![],
            total_tokens: TokenUsage {
                input: 100,
                output: 50,
                cached: 0,
            },
            session_type: "interactive".to_string(),
        }
    }

    #[test]
    fn test_open_memory_success() {
        let db = Database::open_memory().unwrap();
        assert!(db.table_exists("sessions").unwrap());
    }

    #[test]
    fn test_migrate_creates_sessions_table() {
        let db = Database::open_memory().unwrap();
        assert!(db.table_exists("sessions").unwrap());
    }

    #[test]
    fn test_migrate_creates_turns_fts() {
        let db = Database::open_memory().unwrap();
        // FTS tables appear as 'table' in sqlite_master
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='turns_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_schema_version_stored() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 4);
    }

    #[test]
    fn test_migrate_idempotent() {
        let db = Database::open_memory().unwrap();
        // Second migrate call should not error
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), 4);
    }

    // ─── CRUD tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_insert_session_and_exists() {
        let db = Database::open_memory().unwrap();
        let session = make_test_session("sess-001");

        assert!(!db.session_exists("sess-001").unwrap());
        db.insert_session(&session).unwrap();
        assert!(db.session_exists("sess-001").unwrap());
    }

    #[test]
    fn test_insert_session_idempotent() {
        let db = Database::open_memory().unwrap();
        let session = make_test_session("sess-idem");
        db.insert_session(&session).unwrap();
        // INSERT OR IGNORE — second insert must not error
        db.insert_session(&session).unwrap();
        assert_eq!(db.count_sessions().unwrap(), 1);
    }

    #[test]
    fn test_count_sessions() {
        let db = Database::open_memory().unwrap();
        assert_eq!(db.count_sessions().unwrap(), 0);
        db.insert_session(&make_test_session("s1")).unwrap();
        db.insert_session(&make_test_session("s2")).unwrap();
        assert_eq!(db.count_sessions().unwrap(), 2);
    }

    #[test]
    fn test_session_exists_by_prefix() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("abcdef1234567890"))
            .unwrap();
        assert!(db.session_exists_by_prefix("abcdef").unwrap());
        assert!(!db.session_exists_by_prefix("xxxxxx").unwrap());
    }

    #[test]
    fn test_update_vault_path() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-vp")).unwrap();
        db.update_session_vault_path("sess-vp", "raw/sessions/2026-04-01/sess-vp.md")
            .unwrap();
        let paths = db.list_session_vault_paths().unwrap();
        let found = paths.iter().any(|(id, vp)| {
            id == "sess-vp" && vp.as_deref() == Some("raw/sessions/2026-04-01/sess-vp.md")
        });
        assert!(found);
    }

    #[test]
    fn test_update_session_type() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-type")).unwrap();
        db.update_session_type("sess-type", "automated").unwrap();
        let sessions = db.get_all_sessions_for_classify().unwrap();
        let updated = sessions.iter().find(|(id, ..)| id == "sess-type").unwrap();
        assert_eq!(updated.0, "sess-type");
    }

    #[test]
    fn test_delete_session() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-del")).unwrap();
        assert!(db.session_exists("sess-del").unwrap());
        db.delete_session_full("sess-del").unwrap();
        assert!(!db.session_exists("sess-del").unwrap());
    }

    #[test]
    fn test_insert_turn_and_retrieve() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-turn")).unwrap();
        let turn = Turn {
            index: 0,
            role: crate::ingest::Role::User,
            content: "Hello, world!".to_string(),
            timestamp: None,
            actions: vec![],
            thinking: None,
            tokens: None,
            is_sidechain: false,
        };
        db.insert_turn("sess-turn", &turn).unwrap();
        let row = db.get_turn("sess-turn", 0).unwrap();
        assert_eq!(row.content, "Hello, world!");
    }

    #[test]
    fn test_insert_session_from_vault_and_fts() {
        use crate::ingest::markdown::SessionFrontmatter;
        let db = Database::open_memory().unwrap();
        let fm = SessionFrontmatter {
            session_id: "vault-001".to_string(),
            agent: "claude-code".to_string(),
            start_time: "2026-04-01T00:00:00+00:00".to_string(),
            ..Default::default()
        };
        db.insert_session_from_vault(
            &fm,
            "some body text about Rust",
            "raw/sessions/vault-001.md",
        )
        .unwrap();
        assert!(db.session_exists("vault-001").unwrap());
        // FTS row should be present
        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM turns_fts WHERE session_id = 'vault-001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 1);
    }

    // ─── get_sessions_for_date / get_topics_for_sessions ────────────────

    #[test]
    fn test_get_sessions_for_date_filters_by_date() {
        let db = Database::open_memory().unwrap();

        let mut s1 = make_test_session("date-001");
        s1.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 9, 0, 0).unwrap();
        s1.turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: "hello".to_string(),
            actions: vec![],
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        db.insert_session(&s1).unwrap();

        let mut s2 = make_test_session("date-002");
        s2.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 11, 10, 0, 0).unwrap();
        s2.turns = vec![Turn {
            index: 0,
            role: Role::User,
            timestamp: None,
            content: "world".to_string(),
            actions: vec![],
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }];
        db.insert_session(&s2).unwrap();

        let rows = db.get_sessions_for_date("2026-04-10").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "date-001");

        let empty = db.get_sessions_for_date("2026-04-12").unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_get_topics_for_sessions_empty_input() {
        let db = Database::open_memory().unwrap();
        let result = db.get_topics_for_sessions(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_topics_for_sessions_with_edges() {
        let db = Database::open_memory().unwrap();

        // graph_nodes에 먼저 노드 삽입 (FK 제약)
        for (id, ntype, label) in [
            ("session:topic-001", "session", "topic-001"),
            ("topic:rust", "topic", "rust"),
            ("topic:async", "topic", "async"),
            ("file:main.rs", "file", "main.rs"),
        ] {
            db.conn()
                .execute(
                    "INSERT INTO graph_nodes (id, type, label) VALUES (?1, ?2, ?3)",
                    rusqlite::params![id, ntype, label],
                )
                .unwrap();
        }

        // graph_edges 삽입
        db.conn()
            .execute(
                "INSERT INTO graph_edges (source, target, relation, weight) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["session:topic-001", "topic:rust", "discusses_topic", 1.0],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO graph_edges (source, target, relation, weight) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["session:topic-001", "topic:async", "discusses_topic", 0.8],
            )
            .unwrap();
        // 다른 relation은 포함되지 않아야 함
        db.conn()
            .execute(
                "INSERT INTO graph_edges (source, target, relation, weight) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["session:topic-001", "file:main.rs", "modifies_file", 1.0],
            )
            .unwrap();

        let topics = db
            .get_topics_for_sessions(&["topic-001".to_string()])
            .unwrap();
        assert_eq!(topics.len(), 2);
        assert!(topics.iter().all(|(_, t)| t.starts_with("topic:")));
    }

    #[test]
    fn test_delete_session_full_removes_fts() {
        use crate::store::SearchRepo;

        let db = Database::open_memory().unwrap();
        let mut session = make_test_session("sess-fts-del");
        session.turns = vec![
            Turn {
                index: 0,
                role: Role::User,
                content: "first turn content".to_string(),
                timestamp: None,
                actions: vec![],
                thinking: None,
                tokens: None,
                is_sidechain: false,
            },
            Turn {
                index: 1,
                role: Role::Assistant,
                content: "second turn response".to_string(),
                timestamp: None,
                actions: vec![],
                thinking: None,
                tokens: None,
                is_sidechain: false,
            },
        ];
        db.insert_session(&session).unwrap();

        // FTS 행 삽입
        db.insert_fts("first turn content", "sess-fts-del", 0)
            .unwrap();
        db.insert_fts("second turn response", "sess-fts-del", 1)
            .unwrap();

        // FTS 행 존재 확인
        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM turns_fts WHERE session_id = 'sess-fts-del'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 2);

        // delete_session_full 호출
        db.delete_session_full("sess-fts-del").unwrap();

        // FTS 행도 삭제되었는지 확인
        let fts_after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM turns_fts WHERE session_id = 'sess-fts-del'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_after, 0);

        // 세션과 turns도 삭제 확인
        assert!(!db.session_exists("sess-fts-del").unwrap());
    }

    #[test]
    fn test_get_sessions_since_timezone_rfc3339() {
        let db = Database::open_memory().unwrap();

        // s1: UTC 2026-04-09T15:00:00 = KST 2026-04-10 00:00
        let mut s1 = make_test_session("tz-001");
        s1.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 9, 15, 0, 0).unwrap();
        db.insert_session(&s1).unwrap();

        // s2: UTC 2026-04-10T01:00:00
        let mut s2 = make_test_session("tz-002");
        s2.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 1, 0, 0).unwrap();
        db.insert_session(&s2).unwrap();

        // s3: UTC 2026-04-11T00:00:00
        let mut s3 = make_test_session("tz-003");
        s3.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 11, 0, 0, 0).unwrap();
        db.insert_session(&s3).unwrap();

        // KST 2026-04-10 자정 기준 → s1(UTC 4/9 15:00)도 포함되어야 함
        let rows_kst = db.get_sessions_since("2026-04-10T00:00:00+09:00").unwrap();
        assert_eq!(
            rows_kst.len(),
            3,
            "KST 4/10 자정 이후 세션: s1, s2, s3 모두 포함"
        );

        // UTC 2026-04-10 자정 기준 → s1(UTC 4/9 15:00)은 제외
        let rows_utc = db.get_sessions_since("2026-04-10T00:00:00+00:00").unwrap();
        assert_eq!(rows_utc.len(), 2, "UTC 4/10 자정 이후 세션: s2, s3만 포함");
        assert_eq!(rows_utc[0].id, "tz-002");
        assert_eq!(rows_utc[1].id, "tz-003");
    }

    #[test]
    fn test_get_sessions_since_date_only_uses_local_tz() {
        let db = Database::open_memory().unwrap();

        // 로컬 타임존 오프셋 확인
        let local_offset = chrono::Local::now().offset().to_string();

        // 로컬 자정 기준으로 변환되는지 검증 (직접 RFC3339 호출과 비교)
        let mut s1 = make_test_session("tz-local-001");
        s1.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap();
        db.insert_session(&s1).unwrap();

        let date_only = db.get_sessions_since("2026-04-10").unwrap();
        let explicit = db
            .get_sessions_since(&format!("2026-04-10T00:00:00{}", local_offset))
            .unwrap();

        // 날짜-only 입력과 로컬 타임존 명시 입력이 동일한 결과를 반환해야 함
        assert_eq!(date_only.len(), explicit.len());
    }
}
