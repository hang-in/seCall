/**
 * 에이전트 식별 dot — prototype layout.jsx 의 AgentDot 패턴 (Stage 2c).
 *
 * agent 값(production: claude-code / codex / gemini-cli / claude.ai / chatgpt / opencode 등)
 * 별로 design tokens 의 --agent-* 색을 매핑한다. 알 수 없는 agent 는 text-3 회색.
 *
 * 작은 dot 외 fill 로는 사용하지 말 것 — 디자인 시스템상 brand accent 만 fill.
 */

const AGENT_COLOR: Record<string, string> = {
  "claude-code": "var(--agent-claude-code)",
  codex: "var(--agent-codex)",
  "gemini-cli": "var(--agent-gemini)",
  "claude.ai": "var(--agent-claude-ai)",
  chatgpt: "var(--agent-chatgpt)",
};

interface Props {
  agent?: string | null;
  /** px */
  size?: number;
  className?: string;
}

export function AgentDot({ agent, size = 6, className = "" }: Props) {
  const bg = (agent && AGENT_COLOR[agent]) || "var(--text-3)";
  return (
    <span
      className={`inline-block shrink-0 rounded-full ${className}`}
      style={{ width: size, height: size, background: bg }}
      aria-hidden
    />
  );
}
