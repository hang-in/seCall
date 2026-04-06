use std::path::Path;

pub mod claude;
pub mod codex;
pub mod detect;
pub mod gemini;
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
}
