use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize, Serialize, JsonSchema, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WikiSearchMode {
    #[default]
    Keyword,
    Semantic,
    Hybrid,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WikiSearchParams {
    /// Search query matched against wiki filename and content
    pub query: String,
    /// Filter by wiki category: projects, topics, decisions (optional)
    pub category: Option<String>,
    /// Max results (default 5)
    pub limit: Option<usize>,
    /// Search mode: keyword(default), semantic, hybrid
    #[serde(default)]
    pub mode: Option<WikiSearchMode>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GraphQueryParams {
    /// Node ID to query (e.g., "project:tunaflow", "tool:Edit", "session:abc12345")
    pub node_id: String,
    /// Max traversal depth (default: 1)
    pub depth: Option<usize>,
    /// Filter by relation type (e.g., "belongs_to", "uses_tool", "same_project")
    pub relation: Option<String>,
}
