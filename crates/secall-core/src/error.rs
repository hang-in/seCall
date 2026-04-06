use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecallError {
    // --- Store ---
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("database not initialized: run `secall init` first")]
    DatabaseNotInitialized,

    // --- Ingest ---
    #[error("parse error for {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("unsupported file format: {0}")]
    UnsupportedFormat(String),

    // --- Search ---
    #[error("search error: {0}")]
    Search(String),

    #[error("embedding error: {0}")]
    Embedding(#[source] anyhow::Error),

    // --- Vault ---
    #[error("vault I/O error: {0}")]
    VaultIo(#[from] std::io::Error),

    // --- Not Found ---
    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("turn not found: session={session_id} turn={turn_index}")]
    TurnNotFound {
        session_id: String,
        turn_index: u32,
    },

    // --- Config ---
    #[error("config error: {0}")]
    Config(String),

    // --- General (anyhow fallback) ---
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, SecallError>;
