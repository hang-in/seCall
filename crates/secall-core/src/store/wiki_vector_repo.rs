use crate::store::db::Database;
use crate::store::vector_repo::{bytes_to_floats, cosine_distance, floats_to_bytes};

#[derive(Debug, Clone)]
pub struct WikiVectorRow {
    pub wiki_path: String,
    pub embedding: Vec<f32>,
    pub model_id: String,
    pub content_hash: String,
    pub updated_at: String,
}

pub trait WikiVectorRepo {
    fn upsert_wiki_vector(
        &self,
        wiki_path: &str,
        embedding: &[f32],
        model_id: &str,
        content_hash: &str,
    ) -> anyhow::Result<()>;
    fn get_wiki_vector(&self, wiki_path: &str) -> anyhow::Result<Option<WikiVectorRow>>;
    fn list_wiki_vectors(&self) -> anyhow::Result<Vec<WikiVectorRow>>;
    fn delete_wiki_vector(&self, wiki_path: &str) -> anyhow::Result<()>;
}

impl WikiVectorRepo for Database {
    fn upsert_wiki_vector(
        &self,
        wiki_path: &str,
        embedding: &[f32],
        model_id: &str,
        content_hash: &str,
    ) -> anyhow::Result<()> {
        if embedding.is_empty() {
            anyhow::bail!("empty wiki embedding for path={wiki_path}");
        }

        let existing_dim: Option<usize> = self
            .conn()
            .query_row(
                "SELECT LENGTH(embedding) FROM wiki_vectors WHERE model_id = ?1 LIMIT 1",
                [model_id],
                |row| row.get::<_, i64>(0).map(|n| n as usize / 4),
            )
            .ok();

        if let Some(dim) = existing_dim {
            if embedding.len() != dim {
                anyhow::bail!(
                    "wiki embedding dimension mismatch: expected {dim}, got {} ({wiki_path})",
                    embedding.len()
                );
            }
        }

        let bytes = floats_to_bytes(embedding);
        self.conn().execute(
            "INSERT INTO wiki_vectors(wiki_path, embedding, model_id, content_hash, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(wiki_path) DO UPDATE SET
                 embedding = excluded.embedding,
                 model_id = excluded.model_id,
                 content_hash = excluded.content_hash,
                 updated_at = datetime('now')",
            rusqlite::params![wiki_path, bytes, model_id, content_hash],
        )?;
        Ok(())
    }

    fn get_wiki_vector(&self, wiki_path: &str) -> anyhow::Result<Option<WikiVectorRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT wiki_path, embedding, model_id, content_hash, updated_at
             FROM wiki_vectors WHERE wiki_path = ?1",
        )?;
        let mut rows = stmt.query([wiki_path])?;
        if let Some(row) = rows.next()? {
            let bytes: Vec<u8> = row.get(1)?;
            return Ok(Some(WikiVectorRow {
                wiki_path: row.get(0)?,
                embedding: bytes_to_floats(&bytes),
                model_id: row.get(2)?,
                content_hash: row.get(3)?,
                updated_at: row.get(4)?,
            }));
        }
        Ok(None)
    }

    fn list_wiki_vectors(&self) -> anyhow::Result<Vec<WikiVectorRow>> {
        let mut stmt = self.conn().prepare(
            "SELECT wiki_path, embedding, model_id, content_hash, updated_at
             FROM wiki_vectors ORDER BY wiki_path ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let bytes: Vec<u8> = row.get(1)?;
            Ok(WikiVectorRow {
                wiki_path: row.get(0)?,
                embedding: bytes_to_floats(&bytes),
                model_id: row.get(2)?,
                content_hash: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        let mut collected = Vec::new();
        for row in rows {
            match row {
                Ok(row) => collected.push(row),
                Err(err) => {
                    tracing::warn!(error = %err, "wiki_vectors row decode failed; skipping row");
                }
            }
        }

        Ok(collected)
    }

    fn delete_wiki_vector(&self, wiki_path: &str) -> anyhow::Result<()> {
        self.conn()
            .execute("DELETE FROM wiki_vectors WHERE wiki_path = ?1", [wiki_path])?;
        Ok(())
    }
}

pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    1.0 - cosine_distance(a, b)
}
