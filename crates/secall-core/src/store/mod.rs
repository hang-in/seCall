use std::path::PathBuf;

pub mod db;
pub mod schema;
pub mod search_repo;
pub mod session_repo;
pub mod vector_repo;

pub use db::Database;
pub use search_repo::SearchRepo;
pub use session_repo::SessionRepo;
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
