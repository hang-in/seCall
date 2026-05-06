use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use secall_core::{
    search::Embedder,
    store::{Database, WikiVectorRepo},
    wiki::WikiIndexer,
};

struct MockEmbedder {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![0.1, 0.2, 0.3, 0.4])
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut rows = Vec::with_capacity(texts.len());
        for text in texts {
            rows.push(self.embed(text).await?);
        }
        Ok(rows)
    }

    async fn is_available(&self) -> bool {
        true
    }

    fn dimensions(&self) -> usize {
        4
    }

    fn model_name(&self) -> &str {
        "mock-bge"
    }
}

fn write_wiki_page(tmp: &tempfile::TempDir, rel_path: &str, body: &str) {
    let path = tmp.path().join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create wiki parent");
    }
    std::fs::write(path, body).expect("write wiki page");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_index_single_page_inserts_row() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = Database::open_memory().expect("db");
    write_wiki_page(&tmp, "wiki/projects/secall.md", "# seCall\n\nhello");

    let calls = Arc::new(AtomicUsize::new(0));
    let embedder = MockEmbedder {
        calls: Arc::clone(&calls),
    };
    let indexer = WikiIndexer {
        vault_path: tmp.path(),
        db: &db,
        embedder: &embedder,
        model_id: "mock-bge",
    };

    let result = indexer.index_all().await.expect("index");
    let row = db
        .get_wiki_vector("wiki/projects/secall.md")
        .expect("get row")
        .expect("row exists");

    assert_eq!(result.scanned, 1);
    assert_eq!(result.indexed, 1);
    assert_eq!(result.skipped, 0);
    assert!(result.failed.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(row.model_id, "mock-bge");
    assert_eq!(row.embedding, vec![0.1, 0.2, 0.3, 0.4]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unchanged_page_skipped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = Database::open_memory().expect("db");
    write_wiki_page(&tmp, "wiki/projects/secall.md", "# seCall\n\nhello");

    let calls = Arc::new(AtomicUsize::new(0));
    let embedder = MockEmbedder {
        calls: Arc::clone(&calls),
    };
    let indexer = WikiIndexer {
        vault_path: tmp.path(),
        db: &db,
        embedder: &embedder,
        model_id: "mock-bge",
    };

    indexer.index_all().await.expect("first index");
    let result = indexer.index_all().await.expect("second index");

    assert_eq!(result.scanned, 1);
    assert_eq!(result.indexed, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_modified_page_reindexed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = Database::open_memory().expect("db");
    write_wiki_page(&tmp, "wiki/projects/secall.md", "# seCall\n\nhello");

    let calls = Arc::new(AtomicUsize::new(0));
    let embedder = MockEmbedder {
        calls: Arc::clone(&calls),
    };
    let indexer = WikiIndexer {
        vault_path: tmp.path(),
        db: &db,
        embedder: &embedder,
        model_id: "mock-bge",
    };

    indexer.index_all().await.expect("first index");
    write_wiki_page(&tmp, "wiki/projects/secall.md", "# seCall\n\nupdated");
    let result = indexer.index_all().await.expect("second index");

    assert_eq!(result.indexed, 1);
    assert_eq!(result.skipped, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_deleted_file_removes_row() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db = Database::open_memory().expect("db");
    write_wiki_page(&tmp, "wiki/projects/secall.md", "# seCall\n\nhello");

    let calls = Arc::new(AtomicUsize::new(0));
    let embedder = MockEmbedder {
        calls: Arc::clone(&calls),
    };
    let indexer = WikiIndexer {
        vault_path: tmp.path(),
        db: &db,
        embedder: &embedder,
        model_id: "mock-bge",
    };

    indexer.index_all().await.expect("first index");
    std::fs::remove_file(tmp.path().join("wiki/projects/secall.md")).expect("remove wiki file");
    let result = indexer.index_all().await.expect("second index");

    assert_eq!(result.scanned, 0);
    assert_eq!(result.deleted, 1);
    assert!(db
        .get_wiki_vector("wiki/projects/secall.md")
        .expect("get row")
        .is_none());
}
