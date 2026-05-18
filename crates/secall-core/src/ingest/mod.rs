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
pub mod opencode;
pub mod types;

pub use types::{Action, AgentKind, Role, Session, TokenUsage, Turn};

/// P49: secall 이 Claude Code 를 invoke 해 세션 요약을 생성할 때 던지는 프롬프트
/// prefix. 변경 시 wiki 등 생성 코드와 함께 갱신할 것.
const SECALL_SUMMARY_PROMPT_PREFIX: &str =
    "Analyze the following conversation and produce a JSON array of topic-based summaries";

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

/// P49 + P83: 노이즈 세션을 감지한다.
///
/// secall 자체가 Claude Code 또는 codex 를 invoke 해 요약/위키를 생성하는 흐름이
/// `~/.claude/projects/` 또는 `~/.codex/sessions/` 에 또 jsonl 로 남으면서 자기참조
/// ingest 가 발생해 vault 가 거의 동일한 짧은 세션으로 오염됐다. 차단 패턴:
///   1. cwd 가 OS 임시 디렉토리 (`/private/var/folders`, `/var/folders`, `/tmp`)
///   2. 첫 user turn 본문이 secall 의 알려진 summary 프롬프트 prefix 로 시작
///   3. (P83) 첫 user turn 본문이 `wiki::WIKI_INVOCATION_MARKER` 를 포함
///      — `secall wiki update` 가 codex/claude 백엔드 subprocess 호출 시 prompt
///      앞에 prefix 로 추가하는 marker. Issue #82 fix.
///
/// 매치 시 사유 문자열을 반환, 정상 세션은 `None`.
pub fn is_noise_session(session: &Session) -> Option<&'static str> {
    if let Some(cwd) = session.cwd.as_ref() {
        if cwd.starts_with("/private/var/folders/")
            || cwd.starts_with("/var/folders/")
            || cwd.starts_with("/tmp/")
        {
            return Some("tmpdir cwd");
        }
    }

    if let Some(first_user) = session.turns.iter().find(|t| t.role == Role::User) {
        let content_trimmed = first_user.content.trim_start();
        if content_trimmed.starts_with(SECALL_SUMMARY_PROMPT_PREFIX) {
            return Some("secall summary prompt");
        }
        // P83 marker 는 `contains` — codex/claude 가 system prompt 를 앞에 prepend
        // 하는 경우에도 robust. 변수는 일관성 위해 `content_trimmed` 재사용
        // (trim 결과 marker 자체는 변하지 않음).
        if content_trimmed.contains(crate::wiki::WIKI_INVOCATION_MARKER) {
            return Some("secall wiki invocation");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dummy_session(cwd: Option<&str>, first_user_content: Option<&str>) -> Session {
        let turns = if let Some(content) = first_user_content {
            vec![Turn {
                index: 0,
                role: Role::User,
                timestamp: None,
                content: content.to_string(),
                actions: vec![],
                tokens: None,
                thinking: None,
                is_sidechain: false,
            }]
        } else {
            vec![]
        };
        Session {
            id: "t".into(),
            agent: AgentKind::ClaudeCode,
            model: None,
            project: None,
            cwd: cwd.map(std::path::PathBuf::from),
            git_branch: None,
            host: None,
            start_time: Utc::now(),
            end_time: None,
            turns,
            total_tokens: TokenUsage::default(),
            session_type: "interactive".into(),
            archived: false,
            archived_at: None,
        }
    }

    #[test]
    fn is_noise_macos_tmpdir() {
        let s = dummy_session(Some("/private/var/folders/sy/abc/T"), None);
        assert_eq!(is_noise_session(&s), Some("tmpdir cwd"));
    }

    #[test]
    fn is_noise_linux_var_folders() {
        let s = dummy_session(Some("/var/folders/x/y/T"), None);
        assert_eq!(is_noise_session(&s), Some("tmpdir cwd"));
    }

    #[test]
    fn is_noise_tmp() {
        let s = dummy_session(Some("/tmp/foo"), None);
        assert_eq!(is_noise_session(&s), Some("tmpdir cwd"));
    }

    #[test]
    fn is_not_noise_normal_cwd() {
        let s = dummy_session(Some("/Users/me/projects/seCall"), Some("Fix the bug"));
        assert_eq!(is_noise_session(&s), None);
    }

    #[test]
    fn is_noise_secall_summary_prompt() {
        let s = dummy_session(
            Some("/Users/me/projects/seCall"),
            Some("Analyze the following conversation and produce a JSON array of topic-based summaries.\n\nEach element..."),
        );
        assert_eq!(is_noise_session(&s), Some("secall summary prompt"));
    }

    #[test]
    fn is_noise_secall_summary_prompt_with_leading_whitespace() {
        let s = dummy_session(
            Some("/Users/me/projects/seCall"),
            Some("   Analyze the following conversation and produce a JSON array of topic-based summaries"),
        );
        assert_eq!(is_noise_session(&s), Some("secall summary prompt"));
    }

    #[test]
    fn is_not_noise_when_no_cwd_no_user_turn() {
        let s = dummy_session(None, None);
        assert_eq!(is_noise_session(&s), None);
    }

    /// P83 (issue #82): codex/claude wiki 호출이 marker prefix 한 prompt 로
    /// 생성한 세션은 self-ingest 루프 차단을 위해 skip.
    #[test]
    fn is_noise_wiki_invocation_marker_at_start() {
        let prompt = format!(
            "{}\n\nUpdate the wiki for the following sessions...",
            crate::wiki::WIKI_INVOCATION_MARKER
        );
        let s = dummy_session(Some("/Users/me/projects/seCall"), Some(&prompt));
        assert_eq!(is_noise_session(&s), Some("secall wiki invocation"));
    }

    #[test]
    fn is_noise_wiki_invocation_marker_in_middle() {
        // marker 가 어디에 있든 검출되어야 함 (codex/claude 의 system prompt 가
        // 앞에 붙는 경우에도 robust).
        let prompt = format!(
            "Some preamble...\n{}\nMore content",
            crate::wiki::WIKI_INVOCATION_MARKER
        );
        let s = dummy_session(Some("/Users/me/projects/seCall"), Some(&prompt));
        assert_eq!(is_noise_session(&s), Some("secall wiki invocation"));
    }

    #[test]
    fn is_not_noise_without_wiki_marker() {
        let s = dummy_session(
            Some("/Users/me/projects/seCall"),
            Some("Just a normal user prompt without any markers"),
        );
        assert_eq!(is_noise_session(&s), None);
    }
}
