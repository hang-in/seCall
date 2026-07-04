# 06. AI 통합 — Claude Code 핸드오프

## 6.1 진입점
- **홈 화면**: "Create something great." 프롬프트 입력 + **`✨ Create with Claude`**(자연어로 문서/구조 생성) + `ADD A STARTER PACK` + 빈 파일.
- **⌘K 팔레트**: `Open with AI Claude`.

## 6.2 동작 = 로컬 Claude Code 로딩 (★중요)
- 사용자 실측: "Create with Claude"를 누르면 **로컬 Claude Code가 로딩됨**. 즉 OK는 **내장 LLM API로 생성하는 게 아니라, 연결된 에이전트 호스트(Claude Code 등)에 doc 컨텍스트를 핸드오프**함.
- 구조: OK는 **MCP 서버**(`mcp__open-knowledge__*` — exec/search/write/edit/links/workflow 등 19개 도구)를 노출. `ok init`이 에디터(.claude/.cursor/.codex/.opencode)에 MCP 등록 + 스킬(`SKILL.md`+references) 설치. Claude Code가 그 MCP·스킬로 위키를 읽고/쓰며, 결과가 CRDT로 렌더됨.
- 즉 "LLM 연결" = **에이전트가 OK MCP 도구로 위키를 편집** → [03 아티팩트 렌더](03-rich-rendering-artifacts.md)로 폴리시 표시. 이게 "LLM이 위키를 아티팩트처럼" 보이는 파이프라인.

## 6.3 관련 설정
- 설정 Preferences에 **"Open preview when agent edits"** 토글 — 에이전트가 편집할 때마다 프리뷰 자동 갱신.

## 6.4 seCall 적용 (P2)
- seCall은 이미 **AI 세션의 소스 + MCP 서버**를 보유 → 대칭 구조 만들기 쉬움:
  - (a) 위키 문서 생성/편집을 에이전트(Claude Code/로컬 LLM)에 위임하는 진입점(홈 프롬프트 + 문서별 "AI로 편집").
  - (b) 에이전트가 [03]의 컴포넌트(html preview/mermaid/callout)로 결과를 내도록 프롬프트·스킬 유도.
- **차이**: OK는 외부 에이전트 호스트에 의존. seCall은 자체 LLM(Ollama 등)·검색이 있으니 in-app 생성도 가능.
