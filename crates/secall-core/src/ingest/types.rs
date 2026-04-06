use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentKind {
    ClaudeCode,
    Codex,
    GeminiCli,
}

impl AgentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentKind::ClaudeCode => "claude-code",
            AgentKind::Codex => "codex",
            AgentKind::GeminiCli => "gemini-cli",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent: AgentKind,
    pub model: Option<String>,
    pub project: Option<String>,
    pub cwd: Option<PathBuf>,
    pub git_branch: Option<String>,
    pub host: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub turns: Vec<Turn>,
    pub total_tokens: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub index: u32,
    pub role: Role,
    pub timestamp: Option<DateTime<Utc>>,
    pub content: String,
    pub actions: Vec<Action>,
    pub tokens: Option<TokenUsage>,
    pub thinking: Option<String>,
    pub is_sidechain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    ToolUse {
        name: String,
        input_summary: String,
        output_summary: String,
        tool_use_id: Option<String>,
    },
    FileEdit {
        path: String,
    },
    Command {
        cmd: String,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}
