use std::path::Path;

use crate::error::{Result, SecallError};
use crate::ingest::{Session, Turn};
use crate::search::bm25::SessionMeta;
use crate::store::db::{Database, SessionMeta as WikiSessionMeta, TurnRow};

/// (id, project, summary, turn_count, tools_used, session_type)
pub type DailySessionRow = (
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<String>,
    String,
);

pub trait SessionRepo {
    fn insert_session(&self, session: &Session) -> Result<()>;
    fn update_session_vault_path(&self, session_id: &str, vault_path: &str) -> Result<()>;
    fn insert_turn(&self, session_id: &str, turn: &Turn) -> Result<i64>;
    fn session_exists(&self, session_id: &str) -> Result<bool>;
    fn session_exists_by_prefix(&self, prefix: &str) -> Result<bool>;
    fn get_session_meta(&self, session_id: &str) -> Result<SessionMeta>;
    /// 세션이 존재하고 end_time이 NULL이면 true (아직 열린 세션)
    fn is_session_open(&self, session_id: &str) -> Result<bool>;
    /// 세션과 관련 데이터(turns, vectors) 삭제 — 오픈 세션 재인제스트 전 사용
    fn delete_session(&self, session_id: &str) -> Result<()>;
}

// SessionRepo impl for Database — session/turn CRUD
impl SessionRepo for Database {
    fn insert_session(&self, session: &Session) -> crate::error::Result<()> {
        use crate::ingest::markdown::extract_summary;
        use chrono::Utc;
        let tools_used: Vec<String> = session
            .turns
            .iter()
            .flat_map(|t| &t.actions)
            .filter_map(|a| {
                if let crate::ingest::Action::ToolUse { name, .. } = a {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let summary = extract_summary(session);

        self.conn().execute(
            "INSERT OR IGNORE INTO sessions(id, agent, model, project, cwd, git_branch, host, start_time, end_time, turn_count, tokens_in, tokens_out, tools_used, tags, summary, ingested_at, status, session_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            rusqlite::params![
                session.id,
                session.agent.as_str(),
                session.model,
                session.project,
                session.cwd.as_ref().map(|p| p.to_string_lossy().to_string()),
                session.git_branch,
                session.host,
                session.start_time.to_rfc3339(),
                session.end_time.map(|t| t.to_rfc3339()),
                session.turns.len() as i64,
                session.total_tokens.input as i64,
                session.total_tokens.output as i64,
                serde_json::to_string(&tools_used).ok(),
                serde_json::to_string(&Vec::<String>::new()).ok(),
                summary,
                Utc::now().to_rfc3339(),
                "raw",
                &session.session_type,
            ],
        )?;
        Ok(())
    }

    fn update_session_vault_path(
        &self,
        session_id: &str,
        vault_path: &str,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            "UPDATE sessions SET vault_path = ?1, status = 'indexed' WHERE id = ?2",
            rusqlite::params![vault_path, session_id],
        )?;
        Ok(())
    }

    fn insert_turn(
        &self,
        session_id: &str,
        turn: &crate::ingest::Turn,
    ) -> crate::error::Result<i64> {
        let tool_names: Vec<String> = turn
            .actions
            .iter()
            .filter_map(|a| {
                if let crate::ingest::Action::ToolUse { name, .. } = a {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        let has_tool = !tool_names.is_empty();

        self.conn().execute(
            "INSERT OR IGNORE INTO turns(session_id, turn_index, role, timestamp, content, has_tool, tool_names, thinking, tokens_in, tokens_out)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                session_id,
                turn.index as i64,
                turn.role.as_str(),
                turn.timestamp.map(|t| t.to_rfc3339()),
                turn.content,
                has_tool as i64,
                serde_json::to_string(&tool_names).ok(),
                turn.thinking,
                turn.tokens.as_ref().map(|t| t.input as i64).unwrap_or(0),
                turn.tokens.as_ref().map(|t| t.output as i64).unwrap_or(0),
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    fn session_exists(&self, session_id: &str) -> crate::error::Result<bool> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn session_exists_by_prefix(&self, prefix: &str) -> crate::error::Result<bool> {
        let pattern = format!("{}%", prefix);
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id LIKE ?1",
            [pattern],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn is_session_open(&self, session_id: &str) -> crate::error::Result<bool> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND end_time IS NULL",
            [session_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn delete_session(&self, session_id: &str) -> crate::error::Result<()> {
        self.conn()
            .execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
        Ok(())
    }

    fn get_session_meta(&self, session_id: &str) -> crate::error::Result<SessionMeta> {
        self.conn()
            .query_row(
                "SELECT agent, model, project, start_time, vault_path, session_type FROM sessions WHERE id = ?1",
                [session_id],
                |row| {
                    let start_time: String = row.get(3)?;
                    let date = start_time.get(..10).unwrap_or("").to_string();
                    Ok(SessionMeta {
                        agent: row.get(0)?,
                        model: row.get(1)?,
                        project: row.get(2)?,
                        date,
                        vault_path: row.get(4)?,
                        session_type: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    SecallError::SessionNotFound(session_id.to_string())
                }
                _ => SecallError::Database(e),
            })
    }
}

// ─── Additional Database methods (session domain) ────────────────────────────

impl Database {
    /// Get a specific turn by session_id and turn_index
    pub fn get_turn(&self, session_id: &str, turn_index: u32) -> Result<TurnRow> {
        self.conn()
            .query_row(
                "SELECT turn_index, role, content FROM turns WHERE session_id = ?1 AND turn_index = ?2",
                rusqlite::params![session_id, turn_index as i64],
                |row| {
                    Ok(TurnRow {
                        turn_index: row.get::<_, i64>(0)? as u32,
                        role: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => SecallError::TurnNotFound {
                    session_id: session_id.to_string(),
                    turn_index,
                },
                _ => SecallError::Database(e),
            })
    }

    pub fn count_sessions(&self) -> Result<i64> {
        let count = self
            .conn()
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))?;
        Ok(count)
    }

    pub fn list_projects(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT DISTINCT project FROM sessions WHERE project IS NOT NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_agents(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare("SELECT DISTINCT agent FROM sessions")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // ─── Lint helpers ────────────────────────────────────────────────────────

    /// Return vault_path for a single session
    pub fn get_session_vault_path(&self, session_id: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT vault_path FROM sessions WHERE id = ?1")?;
        match stmt.query_row([session_id], |row| row.get::<_, Option<String>>(0)) {
            Ok(vp) => Ok(vp),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Return (session_id, vault_path) for all sessions
    pub fn list_session_vault_paths(&self) -> Result<Vec<(String, Option<String>)>> {
        let mut stmt = self.conn().prepare("SELECT id, vault_path FROM sessions")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count sessions per agent
    pub fn agent_counts(&self) -> Result<std::collections::HashMap<String, usize>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT agent, COUNT(*) FROM sessions GROUP BY agent")?;
        let rows = stmt.query_map([], |row| {
            let agent: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((agent, count as usize))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// 세션과 관련된 모든 데이터를 삭제 (sessions, turns, turn_vectors).
    /// `--force` 재수집 시 기존 데이터를 정리하는 데 사용.
    pub fn delete_session_full(&self, session_id: &str) -> Result<()> {
        self.delete_session_vectors(session_id)?;
        // FTS5 행 삭제 (turns 삭제 전에 수행 — session_id로 매칭)
        self.conn().execute(
            "DELETE FROM turns_fts WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        self.conn().execute(
            "DELETE FROM turns WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        self.conn().execute(
            "DELETE FROM sessions WHERE id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(())
    }

    /// 세션의 모든 벡터를 삭제. 부분 임베딩 정리 및 재임베딩 전 DELETE-first에 사용.
    pub fn delete_session_vectors(&self, session_id: &str) -> Result<usize> {
        // turn_vectors 테이블이 없으면 0 반환 (정상)
        let table_exists: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='turn_vectors'",
            [],
            |r| r.get(0),
        )?;
        if table_exists == 0 {
            return Ok(0);
        }
        let deleted = self.conn().execute(
            "DELETE FROM turn_vectors WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(deleted)
    }

    /// Return all session IDs in the database
    pub fn list_all_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn().prepare("SELECT id FROM sessions")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// session summary 업데이트
    pub fn update_session_summary(&self, session_id: &str, summary: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE sessions SET summary = ?1 WHERE id = ?2",
            rusqlite::params![summary, session_id],
        )?;
        Ok(())
    }

    /// Find session IDs ingested more than once in ingest_log
    pub fn find_duplicate_ingest_entries(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn().prepare(
            "SELECT session_id, COUNT(*) as cnt FROM ingest_log WHERE action='ingest' GROUP BY session_id HAVING cnt > 1",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// 기존 절대경로 vault_path를 상대경로로 변환 (one-time migration)
    pub fn migrate_vault_paths_to_relative(&self, vault_root: &Path) -> Result<usize> {
        let vault_root_str = vault_root.to_string_lossy();
        let prefix = format!("{}/", vault_root_str.trim_end_matches('/'));

        let mut stmt = self
            .conn()
            .prepare("SELECT id, vault_path FROM sessions WHERE vault_path IS NOT NULL")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut migrated = 0;
        for (session_id, vault_path) in &rows {
            if vault_path.starts_with(&prefix) {
                let relative = &vault_path[prefix.len()..];
                self.conn().execute(
                    "UPDATE sessions SET vault_path = ?1 WHERE id = ?2",
                    rusqlite::params![relative, session_id],
                )?;
                migrated += 1;
            }
        }
        Ok(migrated)
    }

    /// vault 마크다운의 frontmatter로 sessions 테이블에 insert.
    /// turns 테이블에는 본문 전체를 단일 FTS 청크로 저장.
    pub fn insert_session_from_vault(
        &self,
        fm: &crate::ingest::markdown::SessionFrontmatter,
        body_text: &str,
        vault_path: &str,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO sessions(
                id, agent, model, project, cwd, git_branch, host,
                start_time, end_time, turn_count, tokens_in, tokens_out,
                tools_used, vault_path, summary, ingested_at, status
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, NULL, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, datetime('now'), 'reindexed'
            )",
            rusqlite::params![
                fm.session_id,
                fm.agent,
                fm.model,
                fm.project,
                fm.cwd,
                fm.host,
                fm.start_time,
                fm.end_time,
                fm.turns.unwrap_or(0),
                fm.tokens_in.unwrap_or(0),
                fm.tokens_out.unwrap_or(0),
                fm.tools_used.as_ref().map(|t| t.join(",")),
                vault_path,
                fm.summary,
            ],
        )?;

        // FTS 인덱싱 — 본문 전체를 하나의 청크로
        if !body_text.trim().is_empty() {
            self.conn().execute(
                "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, 0)",
                rusqlite::params![body_text, fm.session_id],
            )?;
        }

        Ok(())
    }

    /// session_id로 Session 구조체를 재구성 (벡터 임베딩용).
    /// turns 테이블에서 content를 읽어 Session.turns를 채운다.
    pub fn get_session_for_embedding(&self, session_id: &str) -> Result<crate::ingest::Session> {
        use crate::ingest::{AgentKind, Role, Session, TokenUsage, Turn};
        use chrono::DateTime;

        // 세션 메타 조회
        let (
            agent_str,
            model,
            project,
            cwd_str,
            start_time_str,
            end_time_str,
            tokens_in,
            tokens_out,
            session_type,
        ) = self
            .conn()
            .query_row(
                "SELECT agent, model, project, cwd, start_time, end_time, tokens_in, tokens_out, session_type
                 FROM sessions WHERE id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, Option<String>>(8)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    SecallError::SessionNotFound(session_id.to_string())
                }
                _ => SecallError::Database(e),
            })?;

        let agent = match agent_str.as_str() {
            "claude-ai" => AgentKind::ClaudeAi,
            "codex" => AgentKind::Codex,
            "gemini-cli" => AgentKind::GeminiCli,
            "gemini-web" => AgentKind::GeminiWeb,
            "chatgpt" => AgentKind::ChatGpt,
            _ => AgentKind::ClaudeCode,
        };

        let start_time = DateTime::parse_from_rfc3339(&start_time_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let end_time = end_time_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        });

        let cwd = cwd_str.map(std::path::PathBuf::from);

        // turns 조회
        let mut stmt = self.conn().prepare(
            "SELECT turn_index, role, content, timestamp FROM turns
             WHERE session_id = ?1 ORDER BY turn_index ASC",
        )?;
        let turns: Vec<Turn> = stmt
            .query_map([session_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|(idx, role_str, content, ts_str)| {
                let role = match role_str.as_str() {
                    "assistant" => Role::Assistant,
                    "system" => Role::System,
                    _ => Role::User,
                };
                let timestamp = ts_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                });
                Turn {
                    index: idx as u32,
                    role,
                    timestamp,
                    content,
                    actions: Vec::new(),
                    tokens: None,
                    thinking: None,
                    is_sidechain: false,
                }
            })
            .collect();

        Ok(Session {
            id: session_id.to_string(),
            agent,
            model,
            project,
            cwd,
            git_branch: None,
            host: None,
            start_time,
            end_time,
            turns,
            total_tokens: TokenUsage {
                input: tokens_in as u64,
                output: tokens_out as u64,
                cached: 0,
            },
            session_type: session_type.unwrap_or_else(|| "interactive".to_string()),
        })
    }

    /// 전체 세션의 (id, cwd, project, agent, 첫 user turn content) 반환 (backfill용)
    #[allow(clippy::type_complexity)]
    pub fn get_all_sessions_for_classify(
        &self,
    ) -> Result<Vec<(String, Option<String>, Option<String>, String, String)>> {
        let mut stmt = self.conn().prepare(
            "SELECT s.id, s.cwd, s.project, s.agent, COALESCE(t.content, '')
             FROM sessions s
             LEFT JOIN turns t ON t.session_id = s.id AND t.turn_index = (
                 SELECT MIN(t2.turn_index) FROM turns t2
                 WHERE t2.session_id = s.id AND t2.role = 'user'
             )",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 특정 날짜의 세션 목록 조회 (일기 생성용)
    /// Returns: (id, project, summary, turn_count, tools_used, session_type)
    pub fn get_sessions_for_date(
        &self,
        date: &str, // "YYYY-MM-DD"
    ) -> Result<Vec<DailySessionRow>> {
        let pattern = format!("{}%", date);
        let mut stmt = self.conn().prepare(
            "SELECT id, project, summary, turn_count, tools_used, session_type
             FROM sessions
             WHERE start_time LIKE ?1
             ORDER BY start_time",
        )?;
        let rows = stmt
            .query_map([pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)
                        .unwrap_or_else(|_| "interactive".to_string()),
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 세션들의 discusses_topic 엣지 조회 (일기 주제 파악용)
    pub fn get_topics_for_sessions(&self, session_ids: &[String]) -> Result<Vec<(String, String)>> {
        if session_ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: String = session_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sources: Vec<String> = session_ids
            .iter()
            .map(|id| format!("session:{}", id))
            .collect();
        let sql = format!(
            "SELECT source, target FROM graph_edges
             WHERE relation = 'discusses_topic' AND source IN ({})",
            placeholders
        );
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(sources.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 세션의 session_type 업데이트
    pub fn update_session_type(&self, session_id: &str, session_type: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE sessions SET session_type = ?1 WHERE id = ?2",
            rusqlite::params![session_type, session_id],
        )?;
        Ok(())
    }

    /// 세션 메타데이터 + 턴 내용을 한번에 조회 (위키 생성용)
    pub fn get_session_with_turns(
        &self,
        session_id: &str,
    ) -> Result<(WikiSessionMeta, Vec<TurnRow>)> {
        let meta = self.conn().query_row(
            "SELECT id, agent, project, summary, start_time, turn_count, tools_used, session_type
             FROM sessions WHERE id = ?1",
            [session_id],
            |row| {
                Ok(WikiSessionMeta {
                    id: row.get(0)?,
                    agent: row.get(1)?,
                    project: row.get(2)?,
                    summary: row.get(3)?,
                    start_time: row.get(4)?,
                    turn_count: row.get(5)?,
                    tools_used: row.get(6)?,
                    session_type: row.get::<_, Option<String>>(7)?
                        .unwrap_or_else(|| "interactive".to_string()),
                })
            },
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                SecallError::SessionNotFound(session_id.to_string())
            }
            _ => SecallError::Database(e),
        })?;

        let mut stmt = self.conn().prepare(
            "SELECT turn_index, role, content FROM turns
             WHERE session_id = ?1 ORDER BY turn_index ASC",
        )?;
        let turns = stmt
            .query_map([session_id], |row| {
                Ok(TurnRow {
                    turn_index: row.get::<_, i64>(0)? as u32,
                    role: row.get(1)?,
                    content: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok((meta, turns))
    }

    /// since 날짜 이후 세션 목록 (위키 배치 생성용)
    pub fn get_sessions_since(&self, since: &str) -> Result<Vec<WikiSessionMeta>> {
        // 날짜만 입력된 경우 로컬 타임존 자정으로 정규화
        // 예: KST 사용자가 "2026-04-10" 입력 → "2026-04-10T00:00:00+09:00" → UTC 2026-04-09T15:00:00
        let since_normalized = if since.len() == 10 && since.chars().nth(4) == Some('-') {
            let local_offset = chrono::Local::now().offset().to_string();
            format!("{}T00:00:00{}", since, local_offset)
        } else {
            since.to_string()
        };
        // datetime() 함수로 RFC3339 → UTC 변환 후 비교 (Z vs +00:00 사전순 차이 방지)
        let mut stmt = self.conn().prepare(
            "SELECT id, agent, project, summary, start_time, turn_count, tools_used, session_type
             FROM sessions WHERE datetime(start_time) >= datetime(?1) ORDER BY start_time",
        )?;
        let rows = stmt
            .query_map([&since_normalized], |row| {
                Ok(WikiSessionMeta {
                    id: row.get(0)?,
                    agent: row.get(1)?,
                    project: row.get(2)?,
                    summary: row.get(3)?,
                    start_time: row.get(4)?,
                    turn_count: row.get(5)?,
                    tools_used: row.get(6)?,
                    session_type: row
                        .get::<_, Option<String>>(7)?
                        .unwrap_or_else(|| "interactive".to_string()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// 세션의 turn 수를 반환. compact 전후 turn 수 비교에 사용.
    pub fn count_turns_for_session(&self, session_id: &str) -> Result<usize> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM turns WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| r.get(0),
        )?;
        Ok(count as usize)
    }

    /// P34 Task 07: 세션 단위 mini-chart용 통계.
    /// turns 테이블에서 role별 카운트와 tool_names JSON 배열을 집계하여
    /// 상위 빈도 tool 8개까지 반환한다.
    pub fn get_session_stats(&self, session_id: &str) -> Result<SessionStats> {
        // role 카운트
        let mut stmt = self
            .conn()
            .prepare("SELECT role, COUNT(*) FROM turns WHERE session_id = ?1 GROUP BY role")?;
        let mut user = 0i64;
        let mut assistant = 0i64;
        let mut system = 0i64;
        let rows = stmt.query_map(rusqlite::params![session_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows.filter_map(|r| r.ok()) {
            match row.0.as_str() {
                "user" => user = row.1,
                "assistant" => assistant = row.1,
                "system" => system = row.1,
                _ => {}
            }
        }

        // tool 카운트 — turns.tool_names는 JSON 배열
        let mut stmt2 = self
            .conn()
            .prepare("SELECT tool_names FROM turns WHERE session_id = ?1 AND has_tool = 1")?;
        let mut tool_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let rows2 = stmt2.query_map(rusqlite::params![session_id], |r| {
            r.get::<_, Option<String>>(0)
        })?;
        for json_opt in rows2.filter_map(|r| r.ok()).flatten() {
            if let Ok(names) = serde_json::from_str::<Vec<String>>(&json_opt) {
                for name in names {
                    *tool_map.entry(name).or_insert(0) += 1;
                }
            }
        }
        let mut tool_counts: Vec<(String, i64)> = tool_map.into_iter().collect();
        tool_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        tool_counts.truncate(8);

        Ok(SessionStats {
            user_turns: user,
            assistant_turns: assistant,
            system_turns: system,
            tool_counts,
        })
    }

    // ─── REST listing / mutation (P32 Task 02) ─────────────────────────────

    /// 세션 리스트 조회 (페이지네이션 + 다중 필터).
    pub fn list_sessions_filtered(
        &self,
        f: &SessionListFilter,
    ) -> crate::error::Result<SessionListPage> {
        let mut conditions: Vec<String> = vec![
            // automated session_type은 기본 제외 — recall과 일관성
            "session_type != 'automated'".to_string(),
        ];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(p) = &f.project {
            conditions.push("project = ?".to_string());
            params.push(Box::new(p.clone()));
        }
        if let Some(a) = &f.agent {
            conditions.push("agent = ?".to_string());
            params.push(Box::new(a.clone()));
        }
        if let Some(d) = &f.date_from {
            // start_time은 RFC3339. "YYYY-MM-DD" 비교는 prefix LIKE — 단순히 lex 비교 사용
            conditions.push("start_time >= ?".to_string());
            params.push(Box::new(format!("{d}T00:00:00")));
        }
        if let Some(d) = &f.date_to {
            conditions.push("start_time <= ?".to_string());
            params.push(Box::new(format!("{d}T23:59:59")));
        }
        if let Some(t) = &f.tag {
            // tags는 JSON 배열 문자열. "rust" → '%"rust"%' 패턴 LIKE.
            // 부분 매칭 위험 있으나 MVP 허용.
            conditions.push("tags LIKE ?".to_string());
            params.push(Box::new(format!("%\"{}\"%", t.replace('"', "\"\""))));
        }
        // P34 Task 03: 다중 태그 AND 매칭 — 각 태그가 별도 LIKE.
        for t in &f.tags {
            conditions.push("tags LIKE ?".to_string());
            params.push(Box::new(format!("%\"{}\"%", t.replace('"', "\"\""))));
        }
        if let Some(fav) = f.favorite {
            conditions.push("is_favorite = ?".to_string());
            params.push(Box::new(if fav { 1_i64 } else { 0_i64 }));
        }
        if let Some(q) = &f.q {
            // summary LIKE
            conditions.push("(summary LIKE ? OR project LIKE ?)".to_string());
            let pat = format!("%{q}%");
            params.push(Box::new(pat.clone()));
            params.push(Box::new(pat));
        }

        let where_clause = format!("WHERE {}", conditions.join(" AND "));

        let page = f.page.max(1);
        let page_size = f.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        // total
        let total: i64 = {
            let sql = format!("SELECT COUNT(*) FROM sessions {where_clause}");
            let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            self.conn()
                .query_row(&sql, params_ref.as_slice(), |r| r.get(0))?
        };

        // items
        let sql = format!(
            "SELECT id, agent, project, model, start_time, turn_count, summary, tags, is_favorite, session_type, vault_path, notes
             FROM sessions {where_clause}
             ORDER BY start_time DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = self.conn().prepare(&sql)?;
        // Bind filter params + LIMIT/OFFSET
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = params;
        all_params.push(Box::new(page_size as i64));
        all_params.push(Box::new(offset as i64));
        let params_ref: Vec<&dyn rusqlite::ToSql> = all_params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let id: String = row.get(0)?;
            let agent: String = row.get(1)?;
            let project: Option<String> = row.get(2)?;
            let model: Option<String> = row.get(3)?;
            let start_time: String = row.get(4)?;
            let turn_count: i64 = row.get(5)?;
            let summary: Option<String> = row.get(6)?;
            let tags_json: Option<String> = row.get(7)?;
            let is_favorite: i64 = row.get(8).unwrap_or(0);
            let session_type: String = row.get(9)?;
            let vault_path: Option<String> = row.get(10)?;
            let notes: Option<String> = row.get(11).ok().flatten();

            let tags: Vec<String> = tags_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                .unwrap_or_default();

            // start_time이 RFC3339 형식 — 앞 10자가 YYYY-MM-DD
            let date = start_time.chars().take(10).collect::<String>();

            Ok(SessionListItem {
                id,
                agent,
                project,
                model,
                date,
                start_time,
                turn_count,
                summary,
                tags,
                is_favorite: is_favorite != 0,
                session_type,
                vault_path,
                notes,
            })
        })?;

        let items: Vec<SessionListItem> = rows.filter_map(|r| r.ok()).collect();

        Ok(SessionListPage {
            items,
            total,
            page,
            page_size,
        })
    }

    /// 세션 태그 갱신. 정규화 후 반환된 태그를 응답에 사용.
    pub fn update_session_tags(
        &self,
        session_id: &str,
        tags: &[String],
    ) -> crate::error::Result<Vec<String>> {
        let normalized = crate::store::normalize_tags(tags);
        let json = serde_json::to_string(&normalized)
            .map_err(|e| SecallError::Other(anyhow::anyhow!("tags json serialize: {e}")))?;
        let affected = self.conn().execute(
            "UPDATE sessions SET tags = ?1 WHERE id = ?2",
            rusqlite::params![json, session_id],
        )?;
        if affected == 0 {
            return Err(SecallError::SessionNotFound(session_id.to_string()));
        }
        Ok(normalized)
    }

    /// 즐겨찾기 토글.
    pub fn update_session_favorite(
        &self,
        session_id: &str,
        favorite: bool,
    ) -> crate::error::Result<()> {
        let affected = self.conn().execute(
            "UPDATE sessions SET is_favorite = ?1 WHERE id = ?2",
            rusqlite::params![if favorite { 1_i64 } else { 0_i64 }, session_id],
        )?;
        if affected == 0 {
            return Err(SecallError::SessionNotFound(session_id.to_string()));
        }
        Ok(())
    }

    /// P34 Task 00: 세션 노트 갱신. notes는 사용자 free-form markdown.
    /// 빈 문자열도 그대로 저장 (사용자 의도 보존). null이면 NULL로 저장.
    pub fn update_session_notes(
        &self,
        session_id: &str,
        notes: Option<&str>,
    ) -> crate::error::Result<()> {
        let affected = self.conn().execute(
            "UPDATE sessions SET notes = ?1 WHERE id = ?2",
            rusqlite::params![notes, session_id],
        )?;
        if affected == 0 {
            return Err(SecallError::SessionNotFound(session_id.to_string()));
        }
        Ok(())
    }

    /// 단일 세션의 리스트 아이템 메타 — `do_get` 응답에 tags/is_favorite/notes 등 보강에 사용.
    pub fn get_session_list_item(&self, session_id: &str) -> crate::error::Result<SessionListItem> {
        self.conn()
            .query_row(
                "SELECT id, agent, project, model, start_time, turn_count, summary, tags, is_favorite, session_type, vault_path, notes
                 FROM sessions WHERE id = ?1",
                rusqlite::params![session_id],
                |row| {
                    let id: String = row.get(0)?;
                    let agent: String = row.get(1)?;
                    let project: Option<String> = row.get(2)?;
                    let model: Option<String> = row.get(3)?;
                    let start_time: String = row.get(4)?;
                    let turn_count: i64 = row.get(5)?;
                    let summary: Option<String> = row.get(6)?;
                    let tags_json: Option<String> = row.get(7)?;
                    let is_favorite: i64 = row.get(8).unwrap_or(0);
                    let session_type: String = row.get(9)?;
                    let vault_path: Option<String> = row.get(10)?;
                    let notes: Option<String> = row.get(11).ok().flatten();
                    let tags: Vec<String> = tags_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                        .unwrap_or_default();
                    let date = start_time.chars().take(10).collect::<String>();
                    Ok(SessionListItem {
                        id,
                        agent,
                        project,
                        model,
                        date,
                        start_time,
                        turn_count,
                        summary,
                        tags,
                        is_favorite: is_favorite != 0,
                        session_type,
                        vault_path,
                        notes,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    SecallError::SessionNotFound(session_id.to_string())
                }
                _ => SecallError::Database(e),
            })
    }
}

// ─── REST listing types ────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct SessionListFilter {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    /// 단일 태그 (P32 호환). `tags`와 동시 사용 시 AND 매칭.
    pub tag: Option<String>,
    /// 다중 태그 AND 매칭 (P34 신규). 빈 벡터는 영향 없음.
    pub tags: Vec<String>,
    pub favorite: Option<bool>,
    pub q: Option<String>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionListItem {
    pub id: String,
    pub agent: String,
    pub project: Option<String>,
    pub model: Option<String>,
    pub date: String,
    pub start_time: String,
    pub turn_count: i64,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub session_type: String,
    pub vault_path: Option<String>,
    /// P34 Task 00: 사용자 노트 (free-form markdown)
    pub notes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionListPage {
    pub items: Vec<SessionListItem>,
    pub total: i64,
    pub page: usize,
    pub page_size: usize,
}

/// P34 Task 07: 세션 단위 통계 — turn role 분포 + tool 사용 빈도.
/// SessionDetail mini-chart 응답에 사용.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionStats {
    pub user_turns: i64,
    pub assistant_turns: i64,
    pub system_turns: i64,
    /// 상위 빈도 tool name → count (내림차순, 최대 8개)
    pub tool_counts: Vec<(String, i64)>,
}
