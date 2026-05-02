use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;
#[cfg(test)]
use crate::error::SecallError;

use super::schema::{
    CREATE_CONFIG, CREATE_GRAPH_EDGES, CREATE_GRAPH_INDEXES, CREATE_GRAPH_NODES, CREATE_INDEXES,
    CREATE_INGEST_LOG, CREATE_JOBS, CREATE_QUERY_CACHE, CREATE_SESSIONS, CREATE_TURNS,
    CREATE_TURNS_FTS, CURRENT_SCHEMA_VERSION,
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
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;",
        )?;
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

    /// 마이그레이션을 자동으로 적용하지 않고 주어진 Connection을 그대로 감싼 Database 반환.
    /// 마이그레이션 동작 자체를 검증하는 테스트에서 v4 등 임의 스키마를 직접 만든 뒤 사용.
    #[cfg(test)]
    pub(crate) fn from_connection(conn: Connection) -> Self {
        Self { conn }
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
        if current < 5 && !self.column_exists("sessions", "is_favorite")? {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN is_favorite INTEGER DEFAULT 0",
                [],
            )?;
            // 방어적: ALTER TABLE ADD COLUMN DEFAULT가 기존 row에 적용 안 된 경우 보정
            self.conn.execute(
                "UPDATE sessions SET is_favorite = 0 WHERE is_favorite IS NULL",
                [],
            )?;
            self.conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_sessions_favorite ON sessions(is_favorite) WHERE is_favorite = 1;",
            )?;
        }
        if current < 6 {
            self.conn.execute_batch(CREATE_JOBS)?;
            // 시작 시 1회 cleanup: 7일 이상된 완료/실패/중단 jobs 삭제
            self.conn.execute(
                "DELETE FROM jobs WHERE completed_at IS NOT NULL AND completed_at < datetime('now', '-7 days')",
                [],
            )?;
        }
        if current < 7 && !self.column_exists("sessions", "notes")? {
            self.conn
                .execute("ALTER TABLE sessions ADD COLUMN notes TEXT", [])?;
        }
        if current < 8 && !self.column_exists("sessions", "semantic_extracted_at")? {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN semantic_extracted_at INTEGER",
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
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_migrate_idempotent() {
        let db = Database::open_memory().unwrap();
        // Second migrate call should not error
        db.migrate().unwrap();
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_v5_is_favorite_column_exists() {
        let db = Database::open_memory().unwrap();
        assert!(db.column_exists("sessions", "is_favorite").unwrap());
    }

    #[test]
    fn test_v5_migrates_v4_db() {
        // 임시 v4 스키마(without is_favorite) 만들고 Database::open으로 v5 마이그레이션 트리거
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, model TEXT, project TEXT,
                cwd TEXT, git_branch TEXT, start_time TEXT NOT NULL, end_time TEXT,
                turn_count INTEGER DEFAULT 0, tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0, tools_used TEXT, tags TEXT,
                vault_path TEXT, host TEXT, summary TEXT, ingested_at TEXT NOT NULL,
                status TEXT DEFAULT 'raw', session_type TEXT DEFAULT 'interactive'
            );
            CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, role TEXT NOT NULL, timestamp TEXT, content TEXT NOT NULL, has_tool INTEGER DEFAULT 0, tool_names TEXT, thinking TEXT, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0, UNIQUE(session_id, turn_index));
            CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO config(key, value) VALUES ('schema_version', '4');
            INSERT INTO sessions(id, agent, start_time, ingested_at) VALUES ('test1', 'claude-code', '2026-05-01T00:00:00Z', '2026-05-02T00:00:00Z');",
        )
        .unwrap();

        let db = Database::from_connection(conn);
        db.migrate().unwrap();

        assert!(db.column_exists("sessions", "is_favorite").unwrap());
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );
        // 기존 row가 NULL이 아닌 0으로 채워졌는지
        let fav: i64 = db
            .conn()
            .query_row(
                "SELECT is_favorite FROM sessions WHERE id = 'test1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fav, 0);
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

    // ─── REST listing / mutation (P32 Task 02) ─────────────────────────────

    use crate::store::session_repo::SessionListFilter;

    fn default_list_filter() -> SessionListFilter {
        SessionListFilter {
            page: 1,
            page_size: 30,
            ..Default::default()
        }
    }

    #[test]
    fn test_list_sessions_empty() {
        let db = Database::open_memory().unwrap();
        let page = db.list_sessions_filtered(&default_list_filter()).unwrap();
        assert_eq!(page.total, 0);
        assert!(page.items.is_empty());
        assert_eq!(page.page, 1);
        assert_eq!(page.page_size, 30);
    }

    #[test]
    fn test_list_sessions_pagination() {
        let db = Database::open_memory().unwrap();
        for i in 0..5 {
            let mut s = make_test_session(&format!("sess-{i}"));
            s.start_time = chrono::Utc
                .with_ymd_and_hms(2026, 4, 1 + i as u32, 0, 0, 0)
                .unwrap();
            db.insert_session(&s).unwrap();
        }
        let mut f = default_list_filter();
        f.page_size = 2;
        let p1 = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(p1.total, 5);
        assert_eq!(p1.items.len(), 2);
        // ORDER BY start_time DESC — 가장 최근(sess-4)이 첫
        assert_eq!(p1.items[0].id, "sess-4");

        f.page = 3;
        let p3 = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(p3.items.len(), 1);
    }

    #[test]
    fn test_list_sessions_project_filter() {
        let db = Database::open_memory().unwrap();
        let mut s1 = make_test_session("sess-a");
        s1.project = Some("proj-A".to_string());
        let mut s2 = make_test_session("sess-b");
        s2.project = Some("proj-B".to_string());
        db.insert_session(&s1).unwrap();
        db.insert_session(&s2).unwrap();

        let mut f = default_list_filter();
        f.project = Some("proj-A".to_string());
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "sess-a");
    }

    #[test]
    fn test_list_sessions_excludes_automated() {
        let db = Database::open_memory().unwrap();
        let mut s_auto = make_test_session("sess-auto");
        s_auto.session_type = "automated".to_string();
        let s_inter = make_test_session("sess-inter");
        db.insert_session(&s_auto).unwrap();
        db.insert_session(&s_inter).unwrap();

        let page = db.list_sessions_filtered(&default_list_filter()).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "sess-inter");
    }

    #[test]
    fn test_update_session_tags_normalizes() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-t")).unwrap();

        let normalized = db
            .update_session_tags("sess-t", &["Rust".into(), "RUST".into(), "search ".into()])
            .unwrap();
        // BTreeSet 정렬 + dedup
        assert_eq!(normalized, vec!["rust", "search"]);

        // DB에 실제 반영됐는지
        let mut f = default_list_filter();
        f.tag = Some("rust".into());
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].tags, vec!["rust", "search"]);
    }

    #[test]
    fn test_update_session_tags_missing_session() {
        let db = Database::open_memory().unwrap();
        let res = db.update_session_tags("nonexistent", &["rust".into()]);
        assert!(res.is_err());
    }

    #[test]
    fn test_update_session_favorite_toggle() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-f")).unwrap();

        // 기본 false
        let mut f = default_list_filter();
        f.favorite = Some(true);
        assert_eq!(db.list_sessions_filtered(&f).unwrap().total, 0);

        db.update_session_favorite("sess-f", true).unwrap();
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
        assert!(page.items[0].is_favorite);

        db.update_session_favorite("sess-f", false).unwrap();
        f.favorite = Some(false);
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
        assert!(!page.items[0].is_favorite);
    }

    #[test]
    fn test_update_session_favorite_missing_session() {
        let db = Database::open_memory().unwrap();
        assert!(db.update_session_favorite("nonexistent", true).is_err());
    }

    #[test]
    fn test_list_sessions_multi_tag_and() {
        // P34 Task 03: 다중 태그 AND 매칭.
        // sess-both: ["rust", "search"] — 두 태그 모두 가짐 → 매칭
        // sess-rust: ["rust"] — 'search' 태그 없음 → 제외
        // sess-search: ["search"] — 'rust' 태그 없음 → 제외
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-both")).unwrap();
        db.insert_session(&make_test_session("sess-rust")).unwrap();
        db.insert_session(&make_test_session("sess-search"))
            .unwrap();

        db.update_session_tags("sess-both", &["rust".into(), "search".into()])
            .unwrap();
        db.update_session_tags("sess-rust", &["rust".into()])
            .unwrap();
        db.update_session_tags("sess-search", &["search".into()])
            .unwrap();

        let mut f = default_list_filter();
        f.tags = vec!["rust".into(), "search".into()];
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "sess-both");

        // 빈 vector면 영향 없음 (자동 + interactive 세션 3개 모두 반환)
        let mut f_empty = default_list_filter();
        f_empty.tags = vec![];
        let page_all = db.list_sessions_filtered(&f_empty).unwrap();
        assert_eq!(page_all.total, 3);

        // 단일 tag와 다중 tags 동시 사용 — AND 결합
        let mut f_combo = default_list_filter();
        f_combo.tag = Some("rust".into());
        f_combo.tags = vec!["search".into()];
        let page_combo = db.list_sessions_filtered(&f_combo).unwrap();
        assert_eq!(page_combo.total, 1);
        assert_eq!(page_combo.items[0].id, "sess-both");
    }

    #[test]
    fn test_list_sessions_q_filter() {
        let db = Database::open_memory().unwrap();
        let s = make_test_session("sess-q");
        db.insert_session(&s).unwrap();
        // summary는 비어있으므로 q는 project 매칭에 의존
        let mut f = default_list_filter();
        f.q = Some("test-project".into());
        let page = db.list_sessions_filtered(&f).unwrap();
        assert_eq!(page.total, 1);
    }

    // ─── P33 Task 00: jobs 테이블 (v6) ──────────────────────────────────────

    #[test]
    fn test_v6_jobs_table_exists() {
        let db = Database::open_memory().unwrap();
        assert!(db.table_exists("jobs").unwrap());
    }

    #[test]
    fn test_v6_migrates_v5_db() {
        // v5 raw 스키마 (jobs 테이블 없음)에서 마이그레이션이 jobs 테이블을 추가하는지
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, model TEXT, project TEXT,
                cwd TEXT, git_branch TEXT, start_time TEXT NOT NULL, end_time TEXT,
                turn_count INTEGER DEFAULT 0, tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0, tools_used TEXT, tags TEXT,
                vault_path TEXT, host TEXT, summary TEXT, ingested_at TEXT NOT NULL,
                status TEXT DEFAULT 'raw', session_type TEXT DEFAULT 'interactive',
                is_favorite INTEGER DEFAULT 0
            );
            CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, role TEXT NOT NULL, timestamp TEXT, content TEXT NOT NULL, has_tool INTEGER DEFAULT 0, tool_names TEXT, thinking TEXT, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0, UNIQUE(session_id, turn_index));
            CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO config(key, value) VALUES ('schema_version', '5');",
        )
        .unwrap();
        let db = Database::from_connection(conn);
        db.migrate().unwrap();

        assert!(db.table_exists("jobs").unwrap());
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_jobs_insert_and_complete() {
        let db = Database::open_memory().unwrap();
        let metadata = serde_json::json!({"local_only": true, "dry_run": false});
        db.insert_job("job-1", "sync", Some(&metadata)).unwrap();

        let row = db.get_job("job-1").unwrap().expect("inserted job");
        assert_eq!(row.id, "job-1");
        assert_eq!(row.kind, "sync");
        assert_eq!(row.status, "started");
        assert!(row.completed_at.is_none());
        assert_eq!(
            row.metadata
                .as_ref()
                .and_then(|m| m.get("local_only"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );

        let result = serde_json::json!({"ingested": 5});
        db.complete_job("job-1", "completed", Some(&result), None)
            .unwrap();

        let row2 = db.get_job("job-1").unwrap().unwrap();
        assert_eq!(row2.status, "completed");
        assert!(row2.completed_at.is_some());
        assert_eq!(
            row2.result
                .as_ref()
                .and_then(|r| r.get("ingested"))
                .and_then(|v| v.as_i64()),
            Some(5)
        );
        assert!(row2.error.is_none());
    }

    #[test]
    fn test_jobs_complete_with_error() {
        let db = Database::open_memory().unwrap();
        db.insert_job("job-2", "ingest", None).unwrap();
        db.complete_job("job-2", "failed", None, Some("test error"))
            .unwrap();

        let row = db.get_job("job-2").unwrap().unwrap();
        assert_eq!(row.status, "failed");
        assert_eq!(row.error.as_deref(), Some("test error"));
    }

    #[test]
    fn test_jobs_get_missing_returns_none() {
        let db = Database::open_memory().unwrap();
        assert!(db.get_job("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_list_recent_jobs_orders_desc() {
        let db = Database::open_memory().unwrap();
        // 시작 순서대로 insert
        db.insert_job("a", "sync", None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        db.insert_job("b", "sync", None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        db.insert_job("c", "sync", None).unwrap();

        let recent = db.list_recent_jobs(10).unwrap();
        assert_eq!(recent.len(), 3);
        // started_at DESC라 c가 첫번째
        assert_eq!(recent[0].id, "c");
        assert_eq!(recent[1].id, "b");
        assert_eq!(recent[2].id, "a");

        let limited = db.list_recent_jobs(2).unwrap();
        assert_eq!(limited.len(), 2);
    }

    #[test]
    fn test_cleanup_old_jobs() {
        let db = Database::open_memory().unwrap();
        // 옛날 완료 job (8일 전), 최근 완료 job (1일 전), 진행 중 job
        db.conn()
            .execute(
                "INSERT INTO jobs(id, kind, status, started_at, completed_at)
                 VALUES ('old', 'sync', 'completed', datetime('now', '-9 days'), datetime('now', '-8 days')),
                        ('new', 'sync', 'completed', datetime('now', '-2 days'), datetime('now', '-1 days')),
                        ('running', 'sync', 'running', datetime('now', '-1 day'), NULL)",
                [],
            )
            .unwrap();

        let deleted = db.cleanup_old_jobs().unwrap();
        assert_eq!(deleted, 1);

        // 'new'와 'running'만 남음
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        assert!(db.get_job("old").unwrap().is_none());
        assert!(db.get_job("new").unwrap().is_some());
        assert!(db.get_job("running").unwrap().is_some());
    }

    // ─── P34 Task 00: notes 컬럼 (v7) ──────────────────────────────────────

    #[test]
    fn test_v7_notes_column_exists() {
        let db = Database::open_memory().unwrap();
        assert!(db.column_exists("sessions", "notes").unwrap());
    }

    #[test]
    fn test_v7_migrates_v6_db() {
        // v6 raw 스키마 (notes 없음)에서 마이그레이션이 notes 컬럼 추가하는지
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, model TEXT, project TEXT,
                cwd TEXT, git_branch TEXT, start_time TEXT NOT NULL, end_time TEXT,
                turn_count INTEGER DEFAULT 0, tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0, tools_used TEXT, tags TEXT,
                vault_path TEXT, host TEXT, summary TEXT, ingested_at TEXT NOT NULL,
                status TEXT DEFAULT 'raw', session_type TEXT DEFAULT 'interactive',
                is_favorite INTEGER DEFAULT 0
            );
            CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, role TEXT NOT NULL, timestamp TEXT, content TEXT NOT NULL, has_tool INTEGER DEFAULT 0, tool_names TEXT, thinking TEXT, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0, UNIQUE(session_id, turn_index));
            CREATE TABLE jobs (id TEXT PRIMARY KEY, kind TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, completed_at TEXT, error TEXT, result TEXT, metadata TEXT);
            CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO config(key, value) VALUES ('schema_version', '6');",
        )
        .unwrap();
        let db = Database::from_connection(conn);
        db.migrate().unwrap();

        assert!(db.column_exists("sessions", "notes").unwrap());
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_update_session_notes_sets_and_clears() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sess-n")).unwrap();

        // set
        db.update_session_notes("sess-n", Some("hello world"))
            .unwrap();
        let item = db.get_session_list_item("sess-n").unwrap();
        assert_eq!(item.notes.as_deref(), Some("hello world"));

        // clear (None)
        db.update_session_notes("sess-n", None).unwrap();
        let item = db.get_session_list_item("sess-n").unwrap();
        assert!(item.notes.is_none());

        // empty string (사용자 의도 보존)
        db.update_session_notes("sess-n", Some("")).unwrap();
        let item = db.get_session_list_item("sess-n").unwrap();
        assert_eq!(item.notes.as_deref(), Some(""));
    }

    #[test]
    fn test_update_session_notes_missing_session() {
        let db = Database::open_memory().unwrap();
        let res = db.update_session_notes("nonexistent", Some("x"));
        assert!(res.is_err());
    }

    // ─── P34 Task 07: get_session_stats ───────────────────────────────────

    fn make_turn(idx: u32, role: Role, content: &str, tools: &[&str]) -> Turn {
        let actions = tools
            .iter()
            .map(|name| crate::ingest::Action::ToolUse {
                name: (*name).to_string(),
                input_summary: String::new(),
                output_summary: String::new(),
                tool_use_id: None,
            })
            .collect();
        Turn {
            index: idx,
            role,
            timestamp: None,
            content: content.to_string(),
            actions,
            tokens: None,
            thinking: None,
            is_sidechain: false,
        }
    }

    #[test]
    fn test_get_session_stats_role_distribution() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("stats-roles"))
            .unwrap();
        // user 2, assistant 3, system 0
        db.insert_turn("stats-roles", &make_turn(0, Role::User, "u1", &[]))
            .unwrap();
        db.insert_turn("stats-roles", &make_turn(1, Role::Assistant, "a1", &[]))
            .unwrap();
        db.insert_turn("stats-roles", &make_turn(2, Role::User, "u2", &[]))
            .unwrap();
        db.insert_turn("stats-roles", &make_turn(3, Role::Assistant, "a2", &[]))
            .unwrap();
        db.insert_turn("stats-roles", &make_turn(4, Role::Assistant, "a3", &[]))
            .unwrap();

        let stats = db.get_session_stats("stats-roles").unwrap();
        assert_eq!(stats.user_turns, 2);
        assert_eq!(stats.assistant_turns, 3);
        assert_eq!(stats.system_turns, 0);
        assert!(stats.tool_counts.is_empty());
    }

    #[test]
    fn test_get_session_stats_tool_counts() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("stats-tools"))
            .unwrap();

        // turn 0: Read, Edit
        db.insert_turn(
            "stats-tools",
            &make_turn(0, Role::Assistant, "t0", &["Read", "Edit"]),
        )
        .unwrap();
        // turn 1: Read, Bash
        db.insert_turn(
            "stats-tools",
            &make_turn(1, Role::Assistant, "t1", &["Read", "Bash"]),
        )
        .unwrap();
        // turn 2: Read
        db.insert_turn(
            "stats-tools",
            &make_turn(2, Role::Assistant, "t2", &["Read"]),
        )
        .unwrap();
        // turn 3: tool 없음 — has_tool=0, 집계 제외
        db.insert_turn("stats-tools", &make_turn(3, Role::User, "u0", &[]))
            .unwrap();

        let stats = db.get_session_stats("stats-tools").unwrap();
        // Read 3, Edit 1, Bash 1 → 정렬: Read 첫 번째, 이후 (count desc, name asc) → Bash, Edit
        assert_eq!(stats.tool_counts.len(), 3);
        assert_eq!(stats.tool_counts[0], ("Read".to_string(), 3));
        // Edit과 Bash는 count 동일(1) — 이름 오름차순으로 Bash 먼저
        assert_eq!(stats.tool_counts[1], ("Bash".to_string(), 1));
        assert_eq!(stats.tool_counts[2], ("Edit".to_string(), 1));
    }

    #[test]
    fn test_get_session_stats_no_turns_returns_zeros() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("stats-empty"))
            .unwrap();
        let stats = db.get_session_stats("stats-empty").unwrap();
        assert_eq!(stats.user_turns, 0);
        assert_eq!(stats.assistant_turns, 0);
        assert_eq!(stats.system_turns, 0);
        assert!(stats.tool_counts.is_empty());
    }

    // ─── P37 Task 00: semantic_extracted_at 컬럼 (v8) ────────────────────────

    use crate::store::session_repo::GraphRebuildFilter;

    #[test]
    fn test_v8_semantic_extracted_at_column_exists() {
        let db = Database::open_memory().unwrap();
        assert!(db
            .column_exists("sessions", "semantic_extracted_at")
            .unwrap());
    }

    #[test]
    fn test_v8_migrates_v6_db() {
        // v6 raw 스키마(notes/semantic_extracted_at 없음)에서 마이그레이션이
        // 두 컬럼을 모두 추가하고 기존 row 보존하는지
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, agent TEXT NOT NULL, model TEXT, project TEXT,
                cwd TEXT, git_branch TEXT, start_time TEXT NOT NULL, end_time TEXT,
                turn_count INTEGER DEFAULT 0, tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0, tools_used TEXT, tags TEXT,
                vault_path TEXT, host TEXT, summary TEXT, ingested_at TEXT NOT NULL,
                status TEXT DEFAULT 'raw', session_type TEXT DEFAULT 'interactive',
                is_favorite INTEGER DEFAULT 0
            );
            CREATE TABLE turns (id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL, turn_index INTEGER NOT NULL, role TEXT NOT NULL, timestamp TEXT, content TEXT NOT NULL, has_tool INTEGER DEFAULT 0, tool_names TEXT, thinking TEXT, tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0, UNIQUE(session_id, turn_index));
            CREATE TABLE jobs (id TEXT PRIMARY KEY, kind TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL, completed_at TEXT, error TEXT, result TEXT, metadata TEXT);
            CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
            INSERT INTO config(key, value) VALUES ('schema_version', '6');
            INSERT INTO sessions(id, agent, start_time, ingested_at) VALUES ('preserve-1', 'claude-code', '2026-04-01T00:00:00Z', '2026-04-02T00:00:00Z');",
        )
        .unwrap();
        let db = Database::from_connection(conn);
        db.migrate().unwrap();

        // v7 + v8 컬럼 모두 추가됨
        assert!(db.column_exists("sessions", "notes").unwrap());
        assert!(db
            .column_exists("sessions", "semantic_extracted_at")
            .unwrap());
        assert_eq!(
            db.schema_version().unwrap(),
            crate::store::schema::CURRENT_SCHEMA_VERSION
        );

        // 기존 row 보존 확인 — id가 살아있고 semantic_extracted_at은 NULL
        let (id, sem): (String, Option<i64>) = db
            .conn()
            .query_row(
                "SELECT id, semantic_extracted_at FROM sessions WHERE id = 'preserve-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(id, "preserve-1");
        assert!(sem.is_none(), "기존 row의 semantic_extracted_at은 NULL");
    }

    #[test]
    fn test_update_semantic_extracted_at_sets_value() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("sem-1")).unwrap();

        db.update_semantic_extracted_at("sem-1", 1234).unwrap();

        let value: Option<i64> = db
            .conn()
            .query_row(
                "SELECT semantic_extracted_at FROM sessions WHERE id = 'sem-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(value, Some(1234));
    }

    #[test]
    fn test_update_semantic_extracted_at_missing_session_no_op() {
        let db = Database::open_memory().unwrap();
        // 미존재 세션 업데이트는 에러 없이 통과 (0 affected)
        let res = db.update_semantic_extracted_at("nonexistent", 9999);
        assert!(res.is_ok(), "미존재 세션은 에러 안 남");
    }

    #[test]
    fn test_list_sessions_for_graph_rebuild_session_only() {
        let db = Database::open_memory().unwrap();
        db.insert_session(&make_test_session("s-1")).unwrap();
        db.insert_session(&make_test_session("s-2")).unwrap();
        db.insert_session(&make_test_session("s-3")).unwrap();

        let filter = GraphRebuildFilter {
            session: Some("s-2".to_string()),
            // 다른 필드는 무시되어야 함
            all: true,
            retry_failed: true,
            since: Some("2026-01-01".to_string()),
        };
        let ids = db.list_sessions_for_graph_rebuild(filter).unwrap();
        assert_eq!(ids, vec!["s-2"]);

        // 미존재 ID는 빈 결과
        let filter_missing = GraphRebuildFilter {
            session: Some("nonexistent".to_string()),
            ..Default::default()
        };
        let empty = db.list_sessions_for_graph_rebuild(filter_missing).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_list_sessions_for_graph_rebuild_all_overrides_filters() {
        let db = Database::open_memory().unwrap();
        // 3개 세션 — start_time 순서대로
        for (id, day) in [("a-1", 1u32), ("a-2", 2u32), ("a-3", 3u32)] {
            let mut s = make_test_session(id);
            s.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, day, 0, 0, 0).unwrap();
            db.insert_session(&s).unwrap();
        }
        // a-1만 추출 완료 표시 — retry_failed라면 제외되어야 하나, all=true로 포함되어야 함
        db.update_semantic_extracted_at("a-1", 100).unwrap();

        let filter = GraphRebuildFilter {
            all: true,
            retry_failed: true,                    // 무시
            since: Some("2030-01-01".to_string()), // 무시 (정상이라면 0 결과)
            session: None,
        };
        let mut ids = db.list_sessions_for_graph_rebuild(filter).unwrap();
        ids.sort();
        assert_eq!(ids, vec!["a-1", "a-2", "a-3"]);
    }

    #[test]
    fn test_list_sessions_for_graph_rebuild_retry_failed_only_null() {
        let db = Database::open_memory().unwrap();
        for id in ["r-1", "r-2", "r-3"] {
            db.insert_session(&make_test_session(id)).unwrap();
        }
        // r-1, r-3만 추출 완료 — r-2만 NULL 상태 유지
        db.update_semantic_extracted_at("r-1", 111).unwrap();
        db.update_semantic_extracted_at("r-3", 333).unwrap();

        let filter = GraphRebuildFilter {
            retry_failed: true,
            ..Default::default()
        };
        let ids = db.list_sessions_for_graph_rebuild(filter).unwrap();
        assert_eq!(ids, vec!["r-2"]);
    }

    #[test]
    fn test_list_sessions_for_graph_rebuild_since_filters_by_date() {
        let db = Database::open_memory().unwrap();

        // 4/1, 4/5, 4/10 세션
        for (id, day) in [("d-1", 1u32), ("d-5", 5u32), ("d-10", 10u32)] {
            let mut s = make_test_session(id);
            s.start_time = chrono::Utc.with_ymd_and_hms(2026, 4, day, 0, 0, 0).unwrap();
            db.insert_session(&s).unwrap();
        }

        // since=2026-04-05 → d-5, d-10 매칭 (start_time은 RFC3339 "2026-04-05T..." 등)
        // ORDER BY start_time DESC → d-10 먼저
        let filter = GraphRebuildFilter {
            since: Some("2026-04-05".to_string()),
            ..Default::default()
        };
        let ids = db.list_sessions_for_graph_rebuild(filter).unwrap();
        assert_eq!(ids, vec!["d-10", "d-5"]);
    }
}
