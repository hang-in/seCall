---
type: prompt
status: draft
updated_at: 2026-07-06
---

# Sessions 라우트 — Linear 톤 + 반응형 파일럿

## Phase
secall-web UI 고도화 1차 파일럿. 이 결과가 좋으면 다른 라우트(Daily/Wiki/Graph/Commands)로 같은 패턴을 확산한다.

## 배경 (읽고 시작)
- secall-web = React + Vite + Tailwind + **shadcn/ui(이미 셋업됨)** + @tanstack/react-query + react-router.
- 디자인 토큰은 `web/src/index.css` 에 **이미 정교하게 정의돼 있음** (Linear-tone, indigo accent, 라이트/다크):
  - 색 계층: `--bg` / `--surface` / `--surface-2` / `--surface-3` / `--hairline` / `--border-soft` / `--border-strong`
  - 텍스트: `--text` / `--text-2` / `--text-3` / `--text-4`
  - accent: `--accent` / `--accent-hover` / `--accent-soft` / `--accent-border`
  - 타이포: `--t-h1/h2/h3/body/prose/small/meta/caption/mono` (각 -lh, -tr 동반)
  - 여백: `--s-1..--s-10` (8-grid), radius `--r-1..--r-5`
  - **elevation: `--shadow-1/2/pop`**, **motion: `--ease` + `--t-fast(120ms)/--t-base(160ms)/--t-slow(240ms)`**
- Tailwind 에서 이 토큰들은 이미 유틸(`bg-surface`, `text-text-2`, `p-ds-3`, `duration-fast` 등)로 매핑돼 있음 — 기존 컴포넌트 코드에서 사용 패턴을 그대로 참고할 것.
- **문제**: 토큰은 훌륭한데 elevation/motion 이 거의 안 쓰여서 "스켈레톤처럼 밋밋"하고, **반응형 breakpoint 가 lg(1024px) 하나뿐**이라 모바일/태블릿에서 2-pane 이 깨진다.

## 대상 파일 (이것만 수정)
- `web/src/routes/SessionsRoute.tsx` — 2-pane 레이아웃 컨테이너
- `web/src/components/SessionList.tsx` — 좌측 리스트 (가상화/무한스크롤/시맨틱)
- `web/src/components/SessionListItem.tsx` — 리스트 행
- `web/src/components/SessionAside.tsx` — 우측 4-card aside (메타/미니차트/related/notes)
- (필요 시) 세션 상세 본문 컨테이너 컴포넌트

## Focus 1 — Linear 톤 적용 (기존 토큰 활용, 새 토큰 만들지 말 것)
1. **elevation 계층**: 배경(`--bg`) < 카드/리스트(`--surface`) < hover/선택(`--surface-2`) 로 미묘한 밝기 차. 카드·aside 카드에 `--shadow-1`, hover 시 `--shadow-2`. 구분선은 `--hairline` / `--border-soft`.
2. **motion**: 리스트 행·카드·버튼 hover/선택 전환에 `transition` + `--t-fast`/`--t-base` + `--ease`. 선택 행 하이라이트 부드럽게. (진입 애니메이션은 과하지 않게 — 선택 사항)
3. **타이포 위계**: 세션 제목(프로젝트/summary)은 `--t-h3`/`--t-body`, 메타(날짜/turns/agent)는 `--t-meta`/`--t-caption` + `--text-3`. 대비를 명확히.
4. **여백 리듬**: `--s-*` 로 일관된 패딩/갭. 촘촘하되 답답하지 않게 (Linear 밀도감).
5. **focus/hover 상태**: 키보드 focus-visible 링(이미 index.css 에 있음) 유지, hover 상태 시각적 피드백 강화.

## Focus 2 — 반응형 (핵심)
Tailwind breakpoint(`sm=640` `md=768` `lg=1024`)로 3-tier:
- **데스크탑(lg+)**: 현행 2-pane 유지 (좌 리스트 `--list-w` + 우 상세/aside).
- **태블릿(md~lg)**: 2-pane 유지하되 리스트 폭 축소, aside 는 상세 아래로 접거나 폭 축소.
- **모바일(<md)**: **단일 컬럼**. 리스트만 전체 폭으로 보이고, 세션 선택 시 상세가 리스트를 덮는 전체화면(또는 뒤로가기로 리스트 복귀). aside 4-card 는 상세 하단에 세로 스택. 상단 네비는 좁은 화면에서 깨지지 않게.
- 가로 스크롤이 생기지 않도록(overflow-x 방지), 이미지/코드블록/테이블은 자체 스크롤 컨테이너.

## Constraints (반드시 지킬 것)
- **기존 shadcn/ui 컴포넌트(`components/ui/*`)와 index.css 토큰만 사용.** 새 CSS 변수/새 디자인 토큰을 만들지 말 것.
- **도메인 로직(검색/필터/무한스크롤/가상화/삭제 낙관/시맨틱 recall/단축키)은 절대 건드리지 말 것** — 시각/레이아웃만 변경.
- 기존 데이터 훅(useInfiniteSessions/useSemanticRecall/useDeleteSession 등) 시그니처 유지.
- **다크/라이트 둘 다** 정상 (토큰이 이미 양쪽 지원하니 하드코딩 색 쓰지 말 것).
- `tsc --noEmit` 과 `vite build` 통과. 기존 vitest 회귀 없게.
- 커밋하지 말 것 (변경만). 파일 목록과 주요 변경점을 요약해 반환.

## 산출물
- 위 4~5개 파일 수정.
- 무엇을 어떻게 바꿨는지 (elevation/motion/타이포/반응형 각각) 파일:라인 수준 요약.
- tsc/build 통과 여부.
