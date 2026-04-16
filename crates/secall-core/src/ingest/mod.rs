use std::path::Path;

pub mod chatgpt;
pub mod claude;
pub mod claude_ai;
pub mod codex;
pub mod detect;
pub mod gemini;
pub mod gemini_web;
pub mod lint;
pub mod markdown;
pub mod types;

pub use types::{Action, AgentKind, Role, Session, TokenUsage, Turn};

pub trait SessionParser: Send + Sync {
    /// Check if this parser can handle the given path
    fn can_parse(&self, path: &Path) -> bool;

    /// Parse the session file and return a Session
    fn parse(&self, path: &Path) -> crate::error::Result<Session>;

    /// The agent kind this parser handles
    fn agent_kind(&self) -> AgentKind;

    /// Parse a file that may contain multiple sessions (1:N).
    /// Default: wraps parse() for 1:1 parsers.
    fn parse_all(&self, path: &Path) -> crate::error::Result<Vec<Session>> {
        Ok(vec![self.parse(path)?])
    }
}
