use anyhow::Result;
use secall_core::{
    mcp::rest::start_rest_server,
    search::tokenizer::create_tokenizer,
    search::vector::create_vector_indexer,
    search::{Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub async fn run(port: u16) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow::anyhow!("tokenizer init failed: {e}"))?;
    let bm25 = Bm25Indexer::new(tok);
    let vector = create_vector_indexer(&config).await;
    let search = SearchEngine::new(bm25, vector);
    let vault_path = config.vault.path.clone();

    start_rest_server(db, search, vault_path, port).await
}
