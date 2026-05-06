use std::collections::HashSet;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::search::Embedder;
use crate::store::{Database, WikiVectorRepo};

pub struct WikiIndexer<'a> {
    pub vault_path: &'a Path,
    pub db: &'a Database,
    pub embedder: &'a dyn Embedder,
    pub model_id: &'a str,
}

#[derive(Debug, Default)]
pub struct IndexResult {
    pub scanned: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub failed: Vec<(String, String)>,
}

impl<'a> WikiIndexer<'a> {
    pub async fn index_all(&self) -> anyhow::Result<IndexResult> {
        self.index_all_with(false).await
    }

    pub async fn reindex_all(&self) -> anyhow::Result<IndexResult> {
        self.index_all_with(true).await
    }

    async fn index_all_with(&self, force: bool) -> anyhow::Result<IndexResult> {
        let wiki_dir = self.vault_path.join("wiki");
        if !wiki_dir.exists() {
            return Ok(IndexResult::default());
        }

        let mut result = IndexResult::default();
        let mut seen_paths = HashSet::new();

        for entry in walkdir::WalkDir::new(&wiki_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "md")
                    .unwrap_or(false)
            })
        {
            result.scanned += 1;

            let path = entry.path();
            let rel = path
                .strip_prefix(self.vault_path)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            seen_paths.insert(rel.clone());

            match self.index_one(&rel, path, force).await {
                Ok(IndexDisposition::Indexed) => result.indexed += 1,
                Ok(IndexDisposition::Skipped) => result.skipped += 1,
                Err(err) => result.failed.push((rel, err.to_string())),
            }
        }

        for row in self.db.list_wiki_vectors()? {
            if !seen_paths.contains(&row.wiki_path) {
                self.db.delete_wiki_vector(&row.wiki_path)?;
                result.deleted += 1;
            }
        }

        Ok(result)
    }

    async fn index_one(
        &self,
        wiki_path: &str,
        full_path: &Path,
        force: bool,
    ) -> anyhow::Result<IndexDisposition> {
        let content = std::fs::read_to_string(full_path)?;
        let content_hash = hash_content(&content);
        let existing = self.db.get_wiki_vector(wiki_path)?;

        if !force
            && existing
                .as_ref()
                .map(|row| row.content_hash == content_hash && row.model_id == self.model_id)
                .unwrap_or(false)
        {
            return Ok(IndexDisposition::Skipped);
        }

        let embedding = self.embedder.embed(&content).await?;
        self.db
            .upsert_wiki_vector(wiki_path, &embedding, self.model_id, &content_hash)?;
        Ok(IndexDisposition::Indexed)
    }
}

enum IndexDisposition {
    Indexed,
    Skipped,
}

fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}
