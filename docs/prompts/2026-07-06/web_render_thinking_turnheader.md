---
type: prompt
status: draft
updated_at: 2026-07-06
---

# 세션 상세 렌더 개선 — 빈 콜아웃 숨김 + Turn 헤더 토글

secall-web 세션 상세(마크다운) 렌더 2건. 프론트 전용, 즉효.

## 배경
- 세션 상세 본문은 vault `.md`(Obsidian 형식)를 `MarkdownView`(react-markdown + remark/rehype 파이프라인)로 렌더한다.
- 콜아웃 `> [!thinking]- Thinking` / `> [!tool]- Bash` 는 `web/src/lib/remarkObsidianCallouts.ts` 가 `<details class="callout callout-*">` 로 변환한다.
- Turn 구분은 md 안의 `## Turn N — Role` / `### Turn N (HH:MM)` 헤더로 들어온다.

## 문제 & 근본
1. **빈 Thinking 콜아웃**: claude-code 세션의 thinking 이 종종 redacted/빈(`Some("")`)이라, vault md 에 `> [!thinking]- Thinking` **헤더만** 있고 본문이 없다. 웹에서 열면 아무것도 안 나온다(빈 `<details>`).
2. **Turn 헤더 과다**: `### Turn 3 (18:15)` 같은 헤더가 매 턴 나와서 본문 흐름을 끊고 시끄럽다.

## 요구사항

### R1 — 본문 없는 콜아웃 숨김
- `<details class="callout ...">` 중 **summary 를 제외한 실제 본문이 비어있으면 렌더하지 않는다**(또는 시각적으로 감춘다).
- 구현 위치는 `remarkObsidianCallouts.ts`(빈 콜아웃 노드를 아예 만들지 않기)가 가장 깔끔. 파싱 시 callout body 의 텍스트가 공백뿐이면 그 콜아웃을 skip.
- thinking 뿐 아니라 tool 등 모든 타입에 공통 적용(본문 없는 콜아웃은 전부 숨김). 단 tool 콜아웃은 보통 본문(명령/Output)이 있으므로 영향 없어야 한다.
- 주의: "본문 없음" 판정은 summary 라인(`> [!type]- Title`) 다음의 `> ...` 본문 라인들이 전부 빈 경우.

### R2 — Turn 헤더 기본 숨김 + 옵션 토글
- `## Turn N — Role` 및 `### Turn N (HH:MM)` 형태의 **Turn 구분 헤더를 기본적으로 숨긴다**(본문만 자연스럽게 이어지도록).
- 세션 상세 화면 어딘가(헤더 근처)에 **"턴 구분 표시" 토글**(체크박스/스위치, 기본 OFF)을 두고, 켜면 Turn 헤더가 보이게 한다.
- 구현 힌트: MarkdownView 에 `showTurnHeaders?: boolean` prop 추가 → 세션 상세 컴포넌트에서 로컬 state(토글)로 제어. 숨김은 (a) remark 플러그인에서 Turn 헤더 노드 제거 or (b) CSS 로 `h2/h3` 중 Turn 패턴만 감추기 중 안전한 쪽. Turn 헤더 판정은 텍스트가 `Turn <숫자>` 로 시작하는 heading.
- 토글 상태는 localStorage 에 저장해 세션 이동해도 유지되면 좋다(선택).

## 대상 파일 (추정 — 실제는 Read 로 확인)
- `web/src/lib/remarkObsidianCallouts.ts` (R1)
- `web/src/components/MarkdownView.tsx` (R2 prop + Turn 헤더 필터)
- 세션 상세 렌더 컴포넌트(MarkdownView 를 쓰는 곳 — 토글 UI 추가)

## Constraints
- 기존 shadcn/ui + index.css 토큰만. 도메인 로직/데이터 훅 불변.
- 다크/라이트 둘 다. tsc/vite build/vitest 통과. 커밋 금지. 변경 요약 반환.
- **vault md 나 ingest(Rust) 는 건드리지 말 것** — 순수 프론트 렌더 레이어에서만 해결.
