use std::path::PathBuf;

pub mod db;
pub mod graph_repo;
pub mod jobs_repo;
pub mod schema;
pub mod search_repo;
pub mod session_repo;
pub mod tag_normalize;
pub mod vector_repo;

pub use db::Database;
pub use graph_repo::RelatedSession;
pub use jobs_repo::JobRow;
pub use search_repo::SearchRepo;
pub use session_repo::SessionRepo;
pub use tag_normalize::{normalize_tag, normalize_tags};
pub use vector_repo::VectorRepo;

pub fn get_default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("SECALL_DB_PATH") {
        return PathBuf::from(p);
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("secall")
        .join("index.sqlite")
}
