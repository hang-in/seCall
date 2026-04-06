use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum QueryType {
    /// BM25 exact match search
    Keyword,
    /// Vector similarity search (requires embeddings)
    Semantic,
    /// Date filter: today, yesterday, last week, since YYYY-MM-DD
    Temporal,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryItem {
    /// keyword: BM25 exact match. semantic: vector similarity. temporal: date filter
    #[serde(rename = "type")]
    pub query_type: QueryType,
    /// The search query string
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecallParams {
    /// Search queries array — combine keyword, semantic, temporal for best results
    pub queries: Vec<QueryItem>,
    /// Filter by project name
    pub project: Option<String>,
    /// Filter by agent: claude-code, codex, gemini-cli
    pub agent: Option<String>,
    /// Max results (default 10)
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetParams {
    /// Session ID or session_id:turn_index
    pub id: String,
    /// Return full markdown content (default: metadata + summary)
    pub full: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatusParams {}
