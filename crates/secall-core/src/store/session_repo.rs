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
                "SELECT agent, model, project, start_time, vault_path, session_type, is_archived, turn_count FROM sessions WHERE id = ?1",
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
                        is_archived: row.get::<_, i64>(6).unwrap_or(0) != 0,
                        turn_count: row.get::<_, i64>(7).unwrap_or(0),
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

    /// P89 (#100, Gemini PR #101): `(session_id, turn_index)` 다건의 content 를
    /// 단일 쿼리로 가져온다. vector 검색 결과 snippet 채우기의 N+1 회피용.
    ///
    /// 누락된 키 (turn 없음) 는 맵에 포함되지 않는다. 입력이 비면 빈 맵 반환.
    pub fn get_turn_contents(
        &self,
        keys: &[(String, u32)],
    ) -> Result<std::collections::HashMap<(String, u32), String>> {
        use std::collections::HashMap;
        if keys.is_empty() {
            return Ok(HashMap::new());
        }

        // row-value IN: WHERE (session_id, turn_index) IN (VALUES (?,?), (?,?), ...)
        let placeholders = vec!["(?,?)"; keys.len()].join(", ");
        let sql = format!(
            "SELECT session_id, turn_index, content FROM turns \
             WHERE (session_id, turn_index) IN (VALUES {placeholders})"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(keys.len() * 2);
        for (sid, idx) in keys {
            params.push(Box::new(sid.clone()));
            params.push(Box::new(*idx as i64));
        }
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let sid: String = row.get(0)?;
            let idx: i64 = row.get(1)?;
            let content: String = row.get(2)?;
            Ok(((sid, idx as u32), content))
        })?;

        let mut map = HashMap::with_capacity(keys.len());
        for r in rows {
            let (key, content) = r?;
            map.insert(key, content);
        }
        Ok(map)
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

    /// 전체 세션의 태그를 빈도 기준으로 집계.
    /// `sessions.tags`는 JSON 배열 문자열(`'["rust","search"]'`). `json_each`로 펼친 뒤
    /// COUNT(*) 내림차순, 동률이면 태그명 알파벳 오름차순으로 정렬.
    /// `tags`가 NULL이거나 빈 배열인 세션은 결과에 포함되지 않음.
    pub fn list_all_tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self.conn().prepare(
            "SELECT json_each.value AS tag, COUNT(*) AS cnt
             FROM sessions, json_each(sessions.tags)
             WHERE sessions.tags IS NOT NULL AND json_valid(sessions.tags)
             GROUP BY tag
             ORDER BY cnt DESC, tag ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(TagCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
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
        // 지식 그래프 노드/엣지 정리 (`session:{id}`) — DELETE 엔드포인트 및
        // `--force` 재수집에서 고아 그래프 데이터가 남지 않도록 함.
        self.delete_graph_for_session(session_id)?;
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

    /// 세션의 turns_fts 행만 삭제 (sessions/turns/vectors/graph 는 보존).
    /// reindex healing 시 legacy `turn_id=0` 단일 블롭 및 이전 healed FTS 행을
    /// 제거해 per-turn 재삽입이 중복되지 않도록 한다.
    pub fn clear_session_fts(&self, session_id: &str) -> Result<usize> {
        let n = self.conn().execute(
            "DELETE FROM turns_fts WHERE session_id = ?1",
            rusqlite::params![session_id],
        )?;
        Ok(n)
    }

    /// 세션의 turns 행 개수 (healing 대상 판별용).
    pub fn count_session_turns(&self, session_id: &str) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM turns WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// turns 가 0개인 세션 수 (healing 리포트용).
    pub fn count_zero_turn_sessions(&self) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM sessions s WHERE NOT EXISTS \
             (SELECT 1 FROM turns t WHERE t.session_id = s.id)",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
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
        let archived_int: i64 = fm.archived.unwrap_or(false) as i64;
        let archived_at = fm.archived_at.clone();

        self.conn().execute(
            "INSERT OR IGNORE INTO sessions(
                id, agent, model, project, cwd, git_branch, host,
                start_time, end_time, turn_count, tokens_in, tokens_out,
                tools_used, vault_path, summary, ingested_at, status,
                is_archived, archived_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, NULL, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, datetime('now'), 'reindexed',
                ?15, ?16
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
                archived_int,
                archived_at,
            ],
        )?;

        // P45 — 기존 row 가 있던 경우에도 vault frontmatter 의 archive 상태로 DB 동기화.
        // P89 (#100, Gemini PR #101): INSERT OR IGNORE 라 재인덱싱 시 turn_count 가
        // 옛 값으로 남아 RRF 강등 판단이 stale 해진다. archive 와 함께 turn_count 도
        // frontmatter 값으로 동기화.
        self.conn().execute(
            "UPDATE sessions SET is_archived = ?1, archived_at = ?2, turn_count = ?3 WHERE id = ?4",
            rusqlite::params![
                archived_int,
                archived_at,
                fm.turns.unwrap_or(0),
                fm.session_id
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
            archived: false,
            archived_at: None,
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
        date: &str,         // "YYYY-MM-DD" (요청자 로컬 날짜)
        tz_offset_min: i64, // 로컬 - UTC (분). 한국=540. UTC start_time 을 로컬로 이동해 날짜 비교.
    ) -> Result<Vec<DailySessionRow>> {
        let modifier = format!("{} minutes", tz_offset_min);
        let mut stmt = self.conn().prepare(
            "SELECT id, project, summary, turn_count, tools_used, session_type
             FROM sessions
             WHERE DATE(start_time, ?1) = ?2
             ORDER BY start_time",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![modifier, date], |row| {
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

    /// 리스트/달력이 공유하는 세션 필터 술어를 `conditions`/`params` 에 lockstep 으로 push.
    ///
    /// automated/archived 제외 규칙과 project/agent/tag/favorite/q 를 **동일하게** 적용해
    /// 두 뷰(세션 리스트·달력 배지)의 카운트가 조용히 발산하지 않게 한다. 날짜 범위는
    /// 리스트(start_time lexical)와 달력(DATE(start_time, tz) 로컬 날짜)이 의미가 달라
    /// 여기 포함하지 않고 호출 측에서 각각 처리한다.
    fn push_content_conditions(
        f: &SessionListFilter,
        conditions: &mut Vec<String>,
        params: &mut Vec<Box<dyn rusqlite::ToSql>>,
    ) {
        // automated session_type은 기본 제외 — recall과 일관성.
        // Phase 2: include_automated 토글 시에만 포함 (기본 false → 현행 동작 보존).
        if !f.include_automated {
            conditions.push("session_type != 'automated'".to_string());
        }
        if !f.include_archived {
            conditions.push("is_archived = 0".to_string());
        }
        if let Some(p) = &f.project {
            conditions.push("project = ?".to_string());
            params.push(Box::new(p.clone()));
        }
        if let Some(a) = &f.agent {
            conditions.push("agent = ?".to_string());
            params.push(Box::new(a.clone()));
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
    }

    /// 세션 리스트 조회 (페이지네이션 + 다중 필터).
    pub fn list_sessions_filtered(
        &self,
        f: &SessionListFilter,
    ) -> crate::error::Result<SessionListPage> {
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        // 리스트/달력 공통 필터(automated/archived 제외 + project/agent/tag/favorite/q).
        Self::push_content_conditions(f, &mut conditions, &mut params);

        // 날짜 범위 — 리스트 전용(start_time RFC3339 문자열 lexical 비교).
        if let Some(d) = &f.date_from {
            conditions.push("start_time >= ?".to_string());
            params.push(Box::new(format!("{d}T00:00:00")));
        }
        if let Some(d) = &f.date_to {
            conditions.push("start_time <= ?".to_string());
            params.push(Box::new(format!("{d}T23:59:59")));
        }

        // include_automated + include_archived 가 모두 true 이고 다른 필터가 없으면
        // conditions 가 비어 `WHERE ` 만 남는 것을 방지 (기본 경로는 항상 ≥1 조건).
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // ORDER BY — 화이트리스트 컬럼/방향만 (SQL 인젝션 방지). 사용자 입력은
        // enum(SessionSort/SortOrder)으로만 들어오고, 여기서 고정 문자열로 매핑된다.
        let (sort_col, needs_tiebreak) = f.sort.order_column();
        let dir = f.order.sql_keyword();
        let order_clause = if needs_tiebreak {
            // project/agent/turns 는 동률이 흔해 start_time DESC 로 페이지네이션 안정화.
            format!("ORDER BY {sort_col} {dir}, start_time DESC")
        } else {
            format!("ORDER BY {sort_col} {dir}")
        };

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
            "SELECT id, agent, project, model, start_time, turn_count, summary, tags, is_favorite, session_type, vault_path, notes, is_archived, archived_at
             FROM sessions {where_clause}
             {order_clause}
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
            let is_archived: i64 = row.get(12).unwrap_or(0);
            let archived_at: Option<String> = row.get(13).ok().flatten();

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
                is_archived: is_archived != 0,
                archived_at,
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

    /// Phase 3 — 달력 뷰용 날짜별 세션 수.
    ///
    /// `DATE(start_time, tz)` 로 UTC start_time 을 요청자 로컬 날짜로 이동한 뒤
    /// 그 로컬 날짜(YYYY-MM-DD)로 GROUP BY 한다 (#131 데일리와 동일 tz offset 방식).
    /// automated/archived 는 리스트 기본 동작과 동일하게 제외.
    ///
    /// `from`/`to`(YYYY-MM-DD, 로컬 날짜)가 주어지면 그 로컬 날짜 범위로 제한한다.
    pub fn session_calendar_counts(
        &self,
        filter: &SessionListFilter,
        from: Option<&str>,
        to: Option<&str>,
        tz_offset_min: i64, // 로컬 - UTC (분). 한국=540.
    ) -> Result<Vec<CalendarDayCount>> {
        // SQLite DATE modifier 는 `±NNN minutes` 형태를 기대하므로 부호를 명시한다
        // (`{:+}` → "+540"/"-720"). 부호 없는 양수도 대개 동작하나 호환성 위해 명시(리뷰 반영).
        let modifier = format!("{:+} minutes", tz_offset_min);

        // `?` 순서에 맞춰 params 를 lockstep 으로 구성한다. SELECT 의 DATE(start_time, ?)
        // 가 첫 ? 이므로 modifier 를 먼저 bind한 뒤, 리스트와 동일한 필터 술어(공유 헬퍼)를
        // 얹어 배지 카운트가 필터된 리스트와 일치하게 한다. from/to 도 conditions 로 합쳐
        // 필터가 모두 비었을 때 `WHERE` 없이 `AND` 만 남는 구문오류를 방지한다.
        let mut conditions: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(modifier.clone())];
        Self::push_content_conditions(filter, &mut conditions, &mut params);
        if let Some(f) = from {
            conditions.push("DATE(start_time, ?) >= ?".to_string());
            params.push(Box::new(modifier.clone()));
            params.push(Box::new(f.to_string()));
        }
        if let Some(t) = to {
            conditions.push("DATE(start_time, ?) <= ?".to_string());
            params.push(Box::new(modifier.clone()));
            params.push(Box::new(t.to_string()));
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT DATE(start_time, ?) AS d, COUNT(*) AS c FROM sessions \
             {where_clause} GROUP BY d ORDER BY d"
        );

        let conn = self.conn();
        let mut stmt = conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            Ok(CalendarDayCount {
                date: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
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

    /// P37 Task 00: 단일 세션의 `semantic_extracted_at` timestamp 갱신.
    /// 미존재 세션은 0 affected — 호출자가 결과 무시 가능 (에러 안 남).
    pub fn update_semantic_extracted_at(
        &self,
        session_id: &str,
        ts: i64,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            "UPDATE sessions SET semantic_extracted_at = ?1 WHERE id = ?2",
            rusqlite::params![ts, session_id],
        )?;
        Ok(())
    }

    /// P37 Task 00: graph rebuild 처리 대상 세션 ID 목록 반환.
    ///
    /// 우선순위 (위에서 부터 평가하고 첫 매칭만 사용):
    /// 1. `filter.session.is_some()` → 해당 ID만 (단일 row)
    /// 2. `filter.all == true` → 모든 sessions
    /// 3. `filter.retry_failed == true` → `WHERE semantic_extracted_at IS NULL`
    /// 4. `filter.since.is_some()` → `WHERE start_time >= ?`
    /// 5. 기본값 (모든 필드 비활성) → 빈 Vec
    ///
    /// 정렬: `ORDER BY start_time DESC` 일관 적용.
    pub fn list_sessions_for_graph_rebuild(
        &self,
        filter: GraphRebuildFilter,
    ) -> crate::error::Result<Vec<String>> {
        // 1. session ID 단건 조회
        if let Some(id) = filter.session {
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM sessions WHERE id = ?1")?;
            let rows = stmt.query_map([id], |row| row.get::<_, String>(0))?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }

        // 2. all=true → 모든 sessions
        if filter.all {
            let mut stmt = self
                .conn()
                .prepare("SELECT id FROM sessions ORDER BY start_time DESC")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }

        // 3. retry_failed → semantic_extracted_at IS NULL
        if filter.retry_failed {
            let mut stmt = self.conn().prepare(
                "SELECT id FROM sessions
                 WHERE semantic_extracted_at IS NULL
                 ORDER BY start_time DESC",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }

        // 4. since (date 비교, ISO format)
        if let Some(since) = filter.since {
            let mut stmt = self.conn().prepare(
                "SELECT id FROM sessions
                 WHERE start_time >= ?1
                 ORDER BY start_time DESC",
            )?;
            let rows = stmt.query_map([since], |row| row.get::<_, String>(0))?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }

        // 5. 모든 필드 비활성 → 빈 Vec
        Ok(Vec::new())
    }

    /// 단일 세션의 리스트 아이템 메타 — `do_get` 응답에 tags/is_favorite/notes 등 보강에 사용.
    pub fn get_session_list_item(&self, session_id: &str) -> crate::error::Result<SessionListItem> {
        self.conn()
            .query_row(
                "SELECT id, agent, project, model, start_time, turn_count, summary, tags, is_favorite, session_type, vault_path, notes, is_archived, archived_at
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
                    let is_archived: i64 = row.get(12).unwrap_or(0);
                    let archived_at: Option<String> = row.get(13).ok().flatten();
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
                        is_archived: is_archived != 0,
                        archived_at,
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

    /// 세션 archive — DB row 업데이트 + vault frontmatter 갱신.
    /// vault write 실패 시 DB rollback. idempotent.
    pub fn archive_session(
        &self,
        session_id: &str,
        vault: &crate::vault::Vault,
        tz: chrono_tz::Tz,
    ) -> crate::error::Result<()> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        let result = self.conn().query_row(
            "SELECT vault_path, is_archived FROM sessions WHERE id = ?1",
            rusqlite::params![session_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)),
        );

        let (vault_path, current_archived) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(SecallError::SessionNotFound(session_id.to_string()))
            }
            Err(e) => return Err(SecallError::Database(e)),
        };

        if current_archived == 1 {
            return Ok(());
        }

        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE sessions SET is_archived = 1, archived_at = ?1 WHERE id = ?2",
            rusqlite::params![now_str, session_id],
        )?;

        if let Some(rel) = &vault_path {
            vault
                .update_session_archive_frontmatter(rel, true, Some(now), tz)
                .map_err(|e| {
                    SecallError::Config(format!(
                        "vault frontmatter update failed for {session_id}: {e}"
                    ))
                })?;
        }

        tx.commit()?;
        Ok(())
    }

    /// 세션 restore — DB row 업데이트 + vault frontmatter 에서 archived 라인 제거.
    /// idempotent.
    pub fn restore_session(
        &self,
        session_id: &str,
        vault: &crate::vault::Vault,
        tz: chrono_tz::Tz,
    ) -> crate::error::Result<()> {
        let result = self.conn().query_row(
            "SELECT vault_path, is_archived FROM sessions WHERE id = ?1",
            rusqlite::params![session_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, i64>(1)?)),
        );

        let (vault_path, current_archived) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(SecallError::SessionNotFound(session_id.to_string()))
            }
            Err(e) => return Err(SecallError::Database(e)),
        };

        if current_archived == 0 {
            return Ok(());
        }

        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE sessions SET is_archived = 0, archived_at = NULL WHERE id = ?1",
            rusqlite::params![session_id],
        )?;

        if let Some(rel) = &vault_path {
            vault
                .update_session_archive_frontmatter(rel, false, None, tz)
                .map_err(|e| {
                    SecallError::Config(format!(
                        "vault frontmatter restore failed for {session_id}: {e}"
                    ))
                })?;
        }

        tx.commit()?;
        Ok(())
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
    /// P45 — true 면 archived 세션 포함. 기본 false (제외).
    pub include_archived: bool,
    /// Phase 1 — 정렬 기준. 기본 `Date`(= 기존 start_time 정렬).
    pub sort: SessionSort,
    /// Phase 1 — 정렬 방향. 기본 `Desc`(= 기존 최신순).
    pub order: SortOrder,
    /// Phase 2 — true 면 automated 세션 포함. 기본 false (현행 동작 보존).
    pub include_automated: bool,
}

/// Phase 1 — 세션 리스트 정렬 기준. 컬럼명은 `order_column()`에서 화이트리스트
/// 고정 문자열로만 매핑되어 SQL 인젝션이 불가능하다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSort {
    /// start_time (기본)
    #[default]
    Date,
    /// turn_count
    Turns,
    /// project
    Project,
    /// agent
    Agent,
}

impl SessionSort {
    /// 쿼리스트링 값 → enum. 미인식 값은 기본값(Date)으로 폴백해 항상 안전.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "turns" | "turn_count" => Self::Turns,
            "project" => Self::Project,
            "agent" => Self::Agent,
            _ => Self::Date,
        }
    }

    /// (정렬 컬럼, start_time 2차 정렬 필요 여부). 컬럼은 고정 문자열(화이트리스트).
    pub fn order_column(self) -> (&'static str, bool) {
        match self {
            Self::Date => ("start_time", false),
            Self::Turns => ("turn_count", true),
            Self::Project => ("project", true),
            Self::Agent => ("agent", true),
        }
    }
}

/// Phase 1 — 정렬 방향. `sql_keyword()`가 고정 문자열만 반환.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    /// 기본 — 최신/큰 값 우선 (기존 동작 보존)
    #[default]
    Desc,
}

impl SortOrder {
    /// 쿼리스트링 값 → enum. 미인식 값은 기본값(Desc)으로 폴백.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "asc" => Self::Asc,
            _ => Self::Desc,
        }
    }

    fn sql_keyword(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// Phase 3 — 달력 뷰용 날짜별 세션 수. `DATE(start_time, tz)` 로컬 날짜 기준 그룹.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CalendarDayCount {
    /// "YYYY-MM-DD" (요청자 로컬 날짜)
    pub date: String,
    pub count: i64,
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
    /// P45
    pub is_archived: bool,
    pub archived_at: Option<String>,
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

/// P35 Task 00: 태그 + 사용 빈도. `/api/tags` 응답 및
/// `Database::list_all_tags`의 반환 타입.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

/// P37 Task 00: graph rebuild 대상 세션 필터.
///
/// 우선순위: `session` > `all` > `retry_failed` > `since`.
/// 모든 필드 비활성이면 빈 결과 반환 (CLI/REST가 "처리할 세션 없음" 안내).
#[derive(Debug, Default, Clone)]
pub struct GraphRebuildFilter {
    /// "YYYY-MM-DD" 또는 RFC3339. 이 시각 이후 시작된 세션만. None 이면 무시.
    pub since: Option<String>,
    /// 단일 세션 ID. 다른 필터 무시.
    pub session: Option<String>,
    /// true 면 모든 세션 (since/retry_failed 무시). session 보다 우선순위 낮음.
    pub all: bool,
    /// true 면 `semantic_extracted_at IS NULL` 인 세션만.
    pub retry_failed: bool,
}

#[cfg(test)]
mod tests {
    use crate::ingest::markdown::SessionFrontmatter;
    use crate::store::db::Database;
    use crate::store::session_repo::{SessionListFilter, SessionSort, SortOrder};

    fn make_fm(session_id: &str, archived: Option<bool>) -> SessionFrontmatter {
        SessionFrontmatter {
            session_id: session_id.to_string(),
            agent: "claude-code".to_string(),
            date: "2026-05-12".to_string(),
            start_time: "2026-05-12T10:00:00+00:00".to_string(),
            archived,
            archived_at: archived
                .filter(|&a| a)
                .map(|_| "2026-05-12T15:00:00Z".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_insert_session_from_vault_with_archived_sets_db() {
        let db = Database::open_memory().unwrap();
        let fm = make_fm("sess-archived", Some(true));
        db.insert_session_from_vault(&fm, "body text", "raw/sessions/test.md")
            .unwrap();
        let is_archived: i64 = db
            .conn()
            .query_row(
                "SELECT is_archived FROM sessions WHERE id = 'sess-archived'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_archived, 1);
        let archived_at: Option<String> = db
            .conn()
            .query_row(
                "SELECT archived_at FROM sessions WHERE id = 'sess-archived'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(archived_at.is_some());
    }

    #[test]
    fn test_archive_session_sets_db_and_frontmatter() {
        use crate::vault::Vault;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();

        let db = Database::open_memory().unwrap();
        let fm = make_fm("sess-arc-unit", None);
        // vault 파일 먼저 생성
        let md = "---\nsession_id: sess-arc-unit\nagent: claude-code\ndate: 2026-05-12\nstart_time: \"2026-05-12T10:00:00+00:00\"\n---\n\nbody".to_string();
        let rel = "raw/sessions/sess-arc-unit.md";
        std::fs::create_dir_all(dir.path().join("raw/sessions")).unwrap();
        std::fs::write(dir.path().join(rel), &md).unwrap();

        db.insert_session_from_vault(&fm, "body", rel).unwrap();

        db.archive_session("sess-arc-unit", &vault, chrono_tz::UTC)
            .unwrap();

        let is_archived: i64 = db
            .conn()
            .query_row(
                "SELECT is_archived FROM sessions WHERE id = 'sess-arc-unit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_archived, 1);

        let content = std::fs::read_to_string(dir.path().join(rel)).unwrap();
        assert!(content.contains("\narchived: true\n"));
        assert!(content.contains("archived_at:"));
    }

    #[test]
    fn test_restore_session_clears_db_and_frontmatter() {
        use crate::vault::Vault;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();

        let db = Database::open_memory().unwrap();
        let fm = make_fm("sess-rst-unit", None);
        let md = "---\nsession_id: sess-rst-unit\nagent: claude-code\ndate: 2026-05-12\nstart_time: \"2026-05-12T10:00:00+00:00\"\n---\n\nbody".to_string();
        let rel = "raw/sessions/sess-rst-unit.md";
        std::fs::create_dir_all(dir.path().join("raw/sessions")).unwrap();
        std::fs::write(dir.path().join(rel), &md).unwrap();

        db.insert_session_from_vault(&fm, "body", rel).unwrap();
        db.archive_session("sess-rst-unit", &vault, chrono_tz::UTC)
            .unwrap();
        db.restore_session("sess-rst-unit", &vault, chrono_tz::UTC)
            .unwrap();

        let is_archived: i64 = db
            .conn()
            .query_row(
                "SELECT is_archived FROM sessions WHERE id = 'sess-rst-unit'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_archived, 0);

        let content = std::fs::read_to_string(dir.path().join(rel)).unwrap();
        assert!(!content.contains("archived:"));
    }

    #[test]
    fn test_archive_session_unknown_id_returns_error() {
        use crate::vault::Vault;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let vault = Vault::new(dir.path().to_path_buf());
        vault.init().unwrap();
        let db = Database::open_memory().unwrap();

        let result = db.archive_session("nonexistent-id", &vault, chrono_tz::UTC);
        assert!(
            result.is_err(),
            "should return error for unknown session_id"
        );
    }

    #[test]
    fn test_list_sessions_filtered_excludes_archived() {
        let db = Database::open_memory().unwrap();
        let fm_normal = make_fm("sess-list-normal", None);
        db.insert_session_from_vault(&fm_normal, "body", "raw/sessions/n.md")
            .unwrap();
        let fm_arc = make_fm("sess-list-arc", Some(true));
        db.insert_session_from_vault(&fm_arc, "body", "raw/sessions/a.md")
            .unwrap();

        let filter = SessionListFilter {
            page: 1,
            page_size: 100,
            ..Default::default()
        };
        let page = db.list_sessions_filtered(&filter).unwrap();
        assert!(page.items.iter().all(|it| it.id != "sess-list-arc"));
        assert!(page.items.iter().any(|it| it.id == "sess-list-normal"));
    }

    #[test]
    fn test_list_sessions_filtered_include_archived_returns_all() {
        let db = Database::open_memory().unwrap();
        let fm_normal = make_fm("sess-ia-normal", None);
        db.insert_session_from_vault(&fm_normal, "body", "raw/sessions/n2.md")
            .unwrap();
        let fm_arc = make_fm("sess-ia-arc", Some(true));
        db.insert_session_from_vault(&fm_arc, "body", "raw/sessions/a2.md")
            .unwrap();

        let filter = SessionListFilter {
            page: 1,
            page_size: 100,
            include_archived: true,
            ..Default::default()
        };
        let page = db.list_sessions_filtered(&filter).unwrap();
        assert!(page.items.iter().any(|it| it.id == "sess-ia-arc"));
        assert!(page.items.iter().any(|it| it.id == "sess-ia-normal"));
    }

    #[test]
    fn test_insert_session_from_vault_archived_changed_updates_db() {
        let db = Database::open_memory().unwrap();

        // 처음엔 archived=false 로 insert
        let fm_normal = make_fm("sess-reindex", None);
        db.insert_session_from_vault(&fm_normal, "body", "raw/sessions/test2.md")
            .unwrap();

        let is_archived_before: i64 = db
            .conn()
            .query_row(
                "SELECT is_archived FROM sessions WHERE id = 'sess-reindex'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_archived_before, 0);

        // archived=true 로 re-ingest (다른 머신에서 archive 후 git pull 시나리오)
        let fm_archived = make_fm("sess-reindex", Some(true));
        db.insert_session_from_vault(&fm_archived, "body", "raw/sessions/test2.md")
            .unwrap();

        let is_archived_after: i64 = db
            .conn()
            .query_row(
                "SELECT is_archived FROM sessions WHERE id = 'sess-reindex'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_archived_after, 1);
    }

    // Phase 1 — sort/order 화이트리스트 정렬. 기본은 date desc(현행 보존).
    #[test]
    fn test_list_sessions_sort_by_turns_and_default() {
        let db = Database::open_memory().unwrap();
        let mut lo = make_fm("s-turns-lo", None);
        lo.turns = Some(2);
        lo.start_time = "2026-05-11T10:00:00+00:00".to_string();
        let mut hi = make_fm("s-turns-hi", None);
        hi.turns = Some(9);
        hi.start_time = "2026-05-10T10:00:00+00:00".to_string(); // 더 과거
        db.insert_session_from_vault(&lo, "body", "raw/sessions/lo.md")
            .unwrap();
        db.insert_session_from_vault(&hi, "body", "raw/sessions/hi.md")
            .unwrap();

        // 기본값(date desc) — 최신 start_time(lo=05-11) 먼저.
        let def = SessionListFilter {
            page: 1,
            page_size: 100,
            ..Default::default()
        };
        let p = db.list_sessions_filtered(&def).unwrap();
        assert_eq!(p.items[0].id, "s-turns-lo", "기본 정렬은 date desc 여야 함");

        // turns asc — 낮은 turn_count(2) 먼저.
        let asc = SessionListFilter {
            page: 1,
            page_size: 100,
            sort: SessionSort::Turns,
            order: SortOrder::Asc,
            ..Default::default()
        };
        assert_eq!(
            db.list_sessions_filtered(&asc).unwrap().items[0].id,
            "s-turns-lo"
        );

        // turns desc — 높은 turn_count(9) 먼저.
        let desc = SessionListFilter {
            page: 1,
            page_size: 100,
            sort: SessionSort::Turns,
            order: SortOrder::Desc,
            ..Default::default()
        };
        assert_eq!(
            db.list_sessions_filtered(&desc).unwrap().items[0].id,
            "s-turns-hi"
        );
    }

    // Phase 3 — 달력 날짜별 세션 수, tz offset 로컬 날짜 GROUP BY.
    #[test]
    fn test_session_calendar_counts_groups_by_local_date() {
        let db = Database::open_memory().unwrap();
        let mut a = make_fm("cal-a", None);
        a.start_time = "2026-05-12T10:00:00+00:00".to_string(); // KST 05-12 19:00
        let mut b = make_fm("cal-b", None);
        b.start_time = "2026-05-11T20:00:00+00:00".to_string(); // KST 05-12 05:00
        let mut c = make_fm("cal-c", None);
        c.start_time = "2026-05-12T16:00:00+00:00".to_string(); // KST 05-13 01:00
        db.insert_session_from_vault(&a, "body", "raw/sessions/ca.md")
            .unwrap();
        db.insert_session_from_vault(&b, "body", "raw/sessions/cb.md")
            .unwrap();
        db.insert_session_from_vault(&c, "body", "raw/sessions/cc.md")
            .unwrap();

        let all = SessionListFilter::default();
        let days = db.session_calendar_counts(&all, None, None, 540).unwrap();
        let map: std::collections::HashMap<&str, i64> =
            days.iter().map(|d| (d.date.as_str(), d.count)).collect();
        assert_eq!(map.get("2026-05-12"), Some(&2), "KST 05-12 에 a,b 2건");
        assert_eq!(map.get("2026-05-13"), Some(&1), "KST 05-13 에 c 1건");

        // from/to 범위 제한 — 05-13 만.
        let ranged = db
            .session_calendar_counts(&all, Some("2026-05-13"), Some("2026-05-13"), 540)
            .unwrap();
        assert_eq!(ranged.len(), 1);
        assert_eq!(ranged[0].date, "2026-05-13");
        assert_eq!(ranged[0].count, 1);
    }

    // #6 회귀 — 달력 카운트가 리스트와 동일한 필터(automated/archived 제외 + project)를
    // 적용하는지. list 와 별개 하드코딩 WHERE 로 조용히 발산하지 않도록 공유 헬퍼 검증.
    #[test]
    fn test_session_calendar_counts_respects_filters() {
        let db = Database::open_memory().unwrap();
        // 같은 로컬 날짜(KST 05-12)에 interactive(secall)/automated/archived/다른 project.
        let mut a = make_fm("cf-a", None);
        a.project = Some("secall".to_string());
        a.start_time = "2026-05-12T02:00:00+00:00".to_string();
        let mut b = make_fm("cf-auto", None);
        b.project = Some("secall".to_string());
        b.start_time = "2026-05-12T03:00:00+00:00".to_string();
        let mut c = make_fm("cf-arc", Some(true));
        c.project = Some("secall".to_string());
        c.start_time = "2026-05-12T04:00:00+00:00".to_string();
        let mut d = make_fm("cf-other", None);
        d.project = Some("other".to_string());
        d.start_time = "2026-05-12T05:00:00+00:00".to_string();
        db.insert_session_from_vault(&a, "body", "raw/sessions/cf-a.md")
            .unwrap();
        db.insert_session_from_vault(&b, "body", "raw/sessions/cf-auto.md")
            .unwrap();
        db.insert_session_from_vault(&c, "body", "raw/sessions/cf-arc.md")
            .unwrap();
        db.insert_session_from_vault(&d, "body", "raw/sessions/cf-other.md")
            .unwrap();
        // insert 경로가 session_type 을 안 넣으므로 automated 는 직접 지정.
        db.conn()
            .execute(
                "UPDATE sessions SET session_type = 'automated' WHERE id = 'cf-auto'",
                [],
            )
            .unwrap();

        // 기본: automated(cf-auto)/archived(cf-arc) 제외 → a + other = 2.
        let all = SessionListFilter::default();
        let total: i64 = db
            .session_calendar_counts(&all, None, None, 540)
            .unwrap()
            .iter()
            .map(|x| x.count)
            .sum();
        assert_eq!(total, 2, "automated/archived 제외 → 2건");

        // project=secall: other 제외 → a 만 1 (배지가 리스트와 일치해야 함).
        let secall = SessionListFilter {
            project: Some("secall".to_string()),
            ..Default::default()
        };
        let total_secall: i64 = db
            .session_calendar_counts(&secall, None, None, 540)
            .unwrap()
            .iter()
            .map(|x| x.count)
            .sum();
        assert_eq!(total_secall, 1, "달력 배지는 project 필터를 반영해야 함");

        // include_automated=true: a + auto + other = 3 (archived 만 제외).
        let with_auto = SessionListFilter {
            include_automated: true,
            ..Default::default()
        };
        let total_auto: i64 = db
            .session_calendar_counts(&with_auto, None, None, 540)
            .unwrap()
            .iter()
            .map(|x| x.count)
            .sum();
        assert_eq!(total_auto, 3, "include_automated=true → automated 포함");
    }

    // #7 — 정렬 tiebreak: 동률(turn_count) 시 start_time DESC 로 안정 정렬되는지.
    #[test]
    fn test_list_sessions_sort_tiebreak_by_start_time() {
        let db = Database::open_memory().unwrap();
        let mut older = make_fm("tb-older", None);
        older.turns = Some(5);
        older.start_time = "2026-05-10T10:00:00+00:00".to_string();
        let mut newer = make_fm("tb-newer", None);
        newer.turns = Some(5); // 동일 turn_count → tiebreak 발동
        newer.start_time = "2026-05-12T10:00:00+00:00".to_string();
        db.insert_session_from_vault(&older, "body", "raw/sessions/tb-o.md")
            .unwrap();
        db.insert_session_from_vault(&newer, "body", "raw/sessions/tb-n.md")
            .unwrap();

        let asc = SessionListFilter {
            page: 1,
            page_size: 100,
            sort: SessionSort::Turns,
            order: SortOrder::Asc,
            ..Default::default()
        };
        let items = db.list_sessions_filtered(&asc).unwrap().items;
        assert_eq!(
            items[0].id, "tb-newer",
            "동률 turn_count 는 start_time DESC 로 tiebreak → 최신 먼저"
        );
        assert_eq!(items[1].id, "tb-older");
    }

    // #8 — sort=project / sort=agent 컬럼 매핑(화이트리스트) 검증.
    #[test]
    fn test_list_sessions_sort_by_project_and_agent() {
        let db = Database::open_memory().unwrap();
        let mut p1 = make_fm("sp-1", None);
        p1.project = Some("alpha".to_string());
        p1.agent = "gemini-cli".to_string();
        let mut p2 = make_fm("sp-2", None);
        p2.project = Some("zeta".to_string());
        p2.agent = "claude-code".to_string();
        db.insert_session_from_vault(&p1, "body", "raw/sessions/sp1.md")
            .unwrap();
        db.insert_session_from_vault(&p2, "body", "raw/sessions/sp2.md")
            .unwrap();

        let proj_asc = SessionListFilter {
            page: 1,
            page_size: 100,
            sort: SessionSort::Project,
            order: SortOrder::Asc,
            ..Default::default()
        };
        assert_eq!(
            db.list_sessions_filtered(&proj_asc).unwrap().items[0]
                .project
                .as_deref(),
            Some("alpha"),
            "sort=project asc → alpha 먼저"
        );

        let agent_asc = SessionListFilter {
            page: 1,
            page_size: 100,
            sort: SessionSort::Agent,
            order: SortOrder::Asc,
            ..Default::default()
        };
        assert_eq!(
            db.list_sessions_filtered(&agent_asc).unwrap().items[0].agent,
            "claude-code",
            "sort=agent asc → claude-code 먼저"
        );
    }

    // #11 — include_automated + include_archived 모두 true 이고 다른 필터가 없으면
    // conditions 가 비어 WHERE 절이 생략되는 경로. SQL 오류 없이 전체를 반환해야 한다
    // (리스트·달력 공통 경로).
    #[test]
    fn test_list_sessions_all_included_empty_where() {
        let db = Database::open_memory().unwrap();
        let a = make_fm("ew-a", None);
        let arc = make_fm("ew-arc", Some(true));
        db.insert_session_from_vault(&a, "body", "raw/sessions/ew-a.md")
            .unwrap();
        db.insert_session_from_vault(&arc, "body", "raw/sessions/ew-arc.md")
            .unwrap();
        db.conn()
            .execute(
                "UPDATE sessions SET session_type = 'automated' WHERE id = 'ew-a'",
                [],
            )
            .unwrap();

        let all = SessionListFilter {
            page: 1,
            page_size: 100,
            include_automated: true,
            include_archived: true,
            ..Default::default()
        };
        let page = db.list_sessions_filtered(&all).unwrap();
        assert_eq!(
            page.total, 2,
            "필터 전부 해제 시 automated+archived 포함 전체"
        );

        // 달력도 동일한 빈 WHERE 경로에서 SQL 오류 없이 동작해야 한다.
        let cal_total: i64 = db
            .session_calendar_counts(&all, None, None, 540)
            .unwrap()
            .iter()
            .map(|x| x.count)
            .sum();
        assert_eq!(cal_total, 2);
    }

    // P89 (#100, Gemini PR #101): 재인덱싱 시 turn_count 가 frontmatter 값으로
    // 동기화되는지 (INSERT OR IGNORE 로 stale 하게 남지 않는지).
    #[test]
    fn test_insert_session_from_vault_reindex_syncs_turn_count() {
        let db = Database::open_memory().unwrap();

        let read_turn_count = |id: &str| -> i64 {
            db.conn()
                .query_row("SELECT turn_count FROM sessions WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        };

        let mut fm = make_fm("sess-tc", None);
        fm.turns = Some(2);
        db.insert_session_from_vault(&fm, "body", "raw/.sessions/tc.md")
            .unwrap();
        assert_eq!(read_turn_count("sess-tc"), 2);

        // 재인덱싱: turns 2 → 8
        let mut fm2 = make_fm("sess-tc", None);
        fm2.turns = Some(8);
        db.insert_session_from_vault(&fm2, "body", "raw/.sessions/tc.md")
            .unwrap();
        assert_eq!(
            read_turn_count("sess-tc"),
            8,
            "reindex 시 turn_count 가 frontmatter 값으로 동기화돼야 함"
        );
    }
}
