use crate::error::Result;
use crate::search::bm25::{FtsRow, SearchFilters};

pub trait SearchRepo {
    fn insert_fts(&self, tokenized_content: &str, session_id: &str, turn_index: u32)
        -> Result<()>;
    fn search_fts(
        &self,
        tokenized_query: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<Vec<FtsRow>>;
}
