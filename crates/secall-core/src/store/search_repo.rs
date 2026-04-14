use crate::error::{Result, SecallError};
use crate::search::bm25::{FtsRow, SearchFilters};
use crate::store::db::Database;

pub trait SearchRepo {
    fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_index: u32) -> Result<()>;
    fn search_fts(
        &self,
        tokenized_query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<FtsRow>>;
}

// SearchRepo impl for Database — FTS index + search
impl SearchRepo for Database {
    fn insert_fts(
        &self,
        tokenized_content: &str,
        session_id: &str,
        turn_index: u32,
    ) -> crate::error::Result<()> {
        self.conn().execute(
            // FTS5 컬럼명 turn_id는 유지 (스키마 변경 최소화). 저장값은 실제 turn_index.
            "INSERT INTO turns_fts(content, session_id, turn_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![tokenized_content, session_id, turn_index as i64],
        )?;
        Ok(())
    }

    fn search_fts(
        &self,
        tokenized_query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> crate::error::Result<Vec<FtsRow>> {
        let since_str = filters.since.map(|dt| dt.to_rfc3339());
        let until_str = filters.until.map(|dt| dt.to_rfc3339());

        // session_type 제외 조건 — 고정 파라미터 4개 이후부터 ?5, ?6, ...
        let exclude_clause = if filters.exclude_session_types.is_empty() {
            String::new()
        } else {
            let placeholders: String = (0..filters.exclude_session_types.len())
                .map(|i| format!("?{}", i + 5))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "AND (sessions.session_type IS NULL OR sessions.session_type NOT IN ({placeholders}))"
            )
        };

        // session_ids allowlist 조건 — exclude 이후 파라미터 위치
        let base_idx = 5 + filters.exclude_session_types.len();
        let allowlist_clause = match &filters.session_ids_allowlist {
            Some(ids) if ids.is_empty() => {
                // 빈 allowlist → 결과 없음 보장 (caller에서 이미 체크하지만 방어적으로)
                "AND 1=0".to_string()
            }
            Some(ids) => {
                let placeholders: String = (0..ids.len())
                    .map(|i| format!("?{}", i + base_idx))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("AND turns_fts.session_id IN ({placeholders})")
            }
            None => String::new(),
        };

        let sql = format!(
            "SELECT turns_fts.session_id, turns_fts.turn_id, turns_fts.content, bm25(turns_fts) as score
             FROM turns_fts
             JOIN sessions ON turns_fts.session_id = sessions.id
             WHERE turns_fts.content MATCH ?1
               AND (?2 IS NULL OR sessions.start_time >= ?2)
               AND (?3 IS NULL OR sessions.start_time < ?3)
               {exclude_clause}
               {allowlist_clause}
             ORDER BY score
             LIMIT ?4"
        );

        // 고정 파라미터 + exclude_session_types + allowlist 동적 파라미터
        let fixed: Vec<Box<dyn rusqlite::types::ToSql>> = vec![
            Box::new(tokenized_query.to_string()),
            Box::new(since_str),
            Box::new(until_str),
            Box::new(limit as i64),
        ];
        let exclude: Vec<Box<dyn rusqlite::types::ToSql>> = filters
            .exclude_session_types
            .iter()
            .map(|t| -> Box<dyn rusqlite::types::ToSql> { Box::new(t.clone()) })
            .collect();
        let allowlist: Vec<Box<dyn rusqlite::types::ToSql>> = filters
            .session_ids_allowlist
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|id| -> Box<dyn rusqlite::types::ToSql> { Box::new(id.clone()) })
            .collect();

        let all_params: Vec<&dyn rusqlite::types::ToSql> = fixed
            .iter()
            .chain(exclude.iter())
            .chain(allowlist.iter())
            .map(|b| b.as_ref())
            .collect();

        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map(all_params.as_slice(), |row| {
            Ok(FtsRow {
                session_id: row.get(0)?,
                turn_index: row.get::<_, i64>(1)? as u32,
                content: row.get(2)?,
                score: -row.get::<_, f64>(3)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(SecallError::Database)
    }
}

// ─── Additional Database methods (search/FTS domain) ─────────────────────────

impl Database {
    /// Count rows in the turns_fts virtual table
    pub fn count_fts_rows(&self) -> Result<i64> {
        let count = self
            .conn()
            .query_row("SELECT COUNT(*) FROM turns_fts", [], |r| r.get(0))?;
        Ok(count)
    }

    /// Count rows in the turns table
    pub fn count_turns(&self) -> Result<i64> {
        let count = self
            .conn()
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))?;
        Ok(count)
    }

    /// 캐시에서 확장된 쿼리 조회. TTL 7일 초과 시 None.
    pub fn get_query_cache(&self, query: &str) -> Option<String> {
        let hash = Self::query_hash(query);
        self.conn()
            .query_row(
                "SELECT expanded FROM query_cache
                 WHERE query_hash = ?1
                   AND datetime(created_at, '+7 days') > datetime('now')",
                [&hash],
                |row| row.get(0),
            )
            .ok()
    }

    /// 확장 결과를 캐시에 저장.
    pub fn set_query_cache(&self, query: &str, expanded: &str) -> Result<()> {
        let hash = Self::query_hash(query);
        self.conn().execute(
            "INSERT OR REPLACE INTO query_cache(query_hash, original, expanded, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![hash, query, expanded],
        )?;
        Ok(())
    }

    fn query_hash(query: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}
