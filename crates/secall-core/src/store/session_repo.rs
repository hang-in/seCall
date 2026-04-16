use std::path::Path;

use crate::error::{Result, SecallError};
use crate::ingest::{Session, Turn};
use crate::search::bm25::SessionMeta;
use crate::store::db::{Database, SessionMeta as WikiSessionMeta, TurnRow};

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
    ) -> Result<
        Vec<(
            String,
            Option<String>,
            Option<String>,
            i64,
            Option<String>,
            String,
        )>,
    > {
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
}
