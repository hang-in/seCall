---
type: prompt
status: ready
updated_at: 2026-05-06
audience: claude (web/desktop/api) — frontend design assistant
target: secall-web (web/) UI/UX redesign
---

# secall-web 디자인 의뢰 프롬프트 (self-contained)

> 이 파일 한 장만 claude.ai 채팅에 첨부하고 마지막 §11 의 시작 메시지를 그대로 붙여넣으면 디자인 작업이 시작됩니다.
> 별도 스크린샷/reference 첨부 없이 텍스트만으로 완결되도록 작성됐습니다.

---

## 0. 한 줄 요청

당신은 **secall** 의 웹 UI 를 production-grade 수준으로 다시 디자인합니다. 기능 회귀 없이 시각/UX 만 갈아엎습니다. 결과물은 React + Tailwind + shadcn/ui 코드와 짧은 디자인 의도 설명입니다. 톤·색·타이포·간격 등 디자인 결정은 §5 의 방향성을 기준으로 **당신이 알아서 진행**하면 됩니다 — 사용자에게 매번 묻지 마세요.

## 1. Context — secall 이 무엇인지

- **secall** = AI 에이전트(Claude Code / Codex CLI / Gemini CLI / claude.ai / ChatGPT)와의 대화 로그를 로컬에 모아 **위키로 정리하고 검색**하는 도구입니다 (Rust + SQLite + Obsidian 호환 vault).
- 사용자: 터미널 에이전트와 매일 80% 시간을 함께 일하는 개발자. "지난번 그 업스트림 에러 어떻게 패치했더라?" 를 다시 찾는 게 본질 사용 시나리오.
- 본 작업 부분: `web/` — `secall serve --port 8080` 으로 떠올리는 **내장 웹 뷰어**. 로컬 전용. 외부 노출 없음. 단일 사용자.
- 톤: README/커뮤니티 포스트는 캐주얼/자기비하지만, **UI 자체는 차분하고 정제된 프로페셔널 톤**. 내가 매일 들어가서 한참 머무를 도구라는 느낌으로.

## 2. 현재 기술 스택 (변경 X — 그대로 사용)

- React 18 + Vite + TypeScript
- Tailwind CSS + shadcn/ui (CSS variable 기반 design tokens, `--background` / `--foreground` / `--primary` 등)
- 폰트: **Pretendard Variable** (한글) / **Geist Sans** (영문) / **Geist Mono** (코드)
- 다크모드 default (`darkMode: "class"`), 라이트/다크 둘 다 지원해야 함
- 데이터: TanStack Query, Zustand, React Router v6
- 그래프: react-flow + dagre
- 마크다운: react-markdown + rehype/remark
- 빌드 결과물은 `web/dist` → rust binary 에 `rust-embed` 로 임베드. **initial JS ≤ 250 kB gzip 유지**.

## 3. 현재 라우트 / 화면 (텍스트 wireframe)

| Path | 컴포넌트 | 화면 구조 |
|---|---|---|
| `/sessions` | `SessionsRoute` | 좌(SessionList, 무한 스크롤) + 우(상세 placeholder). 상단 SearchBar 에 keyword/semantic 토글 + 태그 필터 + 즐겨찾기 토글 |
| `/sessions/:id` | `SessionDetailRoute` | 상단 헤더(에이전트/프로젝트/날짜/turn 수) + 본문 turns(role 별 prefix) + mini-chart(턴 수/토큰/시간) + RelatedSessions 사이드 + 그래프 fold overlay 진입 |
| `/wiki` `/wiki/:project` | `WikiRoute` | 좌(검색 input + keyword/semantic/hybrid 3-mode 토글 → 결과 또는 wiki 페이지 리스트) + 우(마크다운 본문) |
| `/daily` | `DailyRoute` | 날짜 선택 + 그날의 프로젝트별 세션 묶음 + 자동 생성 일기 |
| `/graph` | `GraphRoute` | 시맨틱 그래프 (react-flow + dagre layout, 노드 타입별 색/아이콘 + legend) |
| `/commands` | `CommandsRoute` | 4개 카드 (Sync / Ingest / Wiki Update / Graph Rebuild) + 옵션 다이얼로그 + 진행률 |

전역:
- `Layout` — 상단 nav (로고 좌측, 라우트 링크, 우측에 dark/light 토글 자리)
- `JobBanner` — 실행 중 job 의 SSE 진행률 + 취소 버튼 (전 화면 상단 고정)
- `JobToastListener` — 완료/실패 토스트
- `HotkeyHelpDialog` — `?` 단축키 도움말 (`/` 검색 포커스, `j/k` 리스트 이동, `g d/w/s/c/g` 라우트 점프, `f` 즐겨찾기, `e` 노트 등)

## 4. "아마추어 느낌" 의 진단 — 무엇을 고쳐야 하는가

다음을 모두 가정하고 일괄 정리해주세요:

1. **시각 hierarchy 약함** — 모든 텍스트가 비슷한 weight/size 라 눈이 어디 가야 할지 모름. 헤더-제목-본문-meta 의 계층이 명확해야 함.
2. **공간 리듬 부재** — 컴포넌트마다 padding/gap 이 (3 / 4 / 6) 섞여 있음. **8px 그리드** 로 통일.
3. **Border radius / elevation / divider 불일치** — 카드, 버튼, 입력의 둥글기/테두리가 제각각.
4. **컬러 팔레트 단조** — shadcn/ui 기본을 거의 그대로 → 차별성 X. 차분한 무채색 베이스 + **단 하나의 accent** 로 정리. 다크모드에서 강한 흰색 대신 **off-white** 톤 (eye fatigue 회피).
5. **Typography** — 한글(Pretendard) / 영문(Geist) 가 한 줄에 섞일 때 line-height/letter-spacing 깨짐. type scale 재정의 (h1/h2/h3/body/caption/mono) + Pretendard 한글 metric 에 맞춘 line-height.
6. **Empty / loading / error / first-run state 디테일 부재** — 그냥 spinner + "불러오는 중…". 상태별 illustration 또는 carefully crafted text + 다음 행동 유도 (CTA).
7. **Micro-interaction 거의 없음** — hover/focus/active 차이 미미. focus ring + 100~150ms 의 미세 transition + 핵심 포인트에서 motion (예: 검색 결과 fade-in, JobBanner pulse).
8. **정보 밀도 vs 여백** — `/sessions` 의 좌측 SessionList 는 dense 하게(한 화면 30+ 항목), 우측 detail 은 reading-friendly (max-width ~720px, 본문 typography prose) 로 명확히 분리.
9. **단축키/메뉴 노출 부족** — `?` 단축키 도움말이 있는 줄도 모를 가능성. 상단 nav 에 작은 키보드 아이콘 + tooltip.
10. **다크모드 default 인데 라이트모드가 안 어울림** — 두 모드 모두 contrast/temperature 통일된 디자인 토큰 필요.

## 5. 방향성 — **확정**

**A. Calm / Editorial** 톤으로 진행합니다.

레퍼런스 (직접 첨부 X, 머릿속에서 톤 차용):

- **Linear (linear.app)** — 작업/이슈 도구의 모범. 차분한 무채색, hairline divider, 좁고 명확한 타이포 스케일, 절제된 motion. **이게 1순위 톤**.
- **Vercel dashboard / Tailscale admin** — 데이터/상태가 많은 도구의 정보 정렬, 카드 레이아웃, mono font 활용.
- **Cursor / Warp 의 메뉴/팔레트** — 코드 도구 특유의 키보드-우선 UX, 단축키 visibility, 좁은 highlight.
- **Obsidian (knowledge vault)** — 사이드바 + content + meta 3-pane 의 정보 흐름. (단 Obsidian 자체의 시각 톤보다는 layout 구조만 차용)

피해야 할 톤:

- 화려한 SaaS landing page 풍 — gradient mesh, glow, drop shadow 남발, 큰 hero 타이포, marketing 카피.
- Material Design 의 무거운 elevation·강한 컬러 일관 — 이 도구는 personal local tool 이라 무겁지 않게.
- "AI 느낌" 을 강조하려는 보라색 그라데이션 / glow / orb. **secall 은 그냥 도구다.**

색 가이드 (확정 — 이 위에서 본인이 마이크로 튜닝):

- 다크: 배경 `#0B0C0D ~ #111316` 사이의 **거의 검정** + 카드/패널은 `#15171A` 톤. 텍스트는 순백색 X, **`#E6E7EA` 정도의 off-white**. 보조 텍스트 `#9AA0A6` 톤.
- 라이트: 배경 거의 흰색 (`#FAFAFA` 톤) + 패널 `#F2F3F5`. 텍스트 `#0F1115`. 보조 `#5F6573`.
- Accent **단 하나** — 한국어 도구 + "검색/위키" 라는 본질에 맞게 **단단한 청록/그린 (teal-ish, 예: `#1FB6A8` 또는 `#2EB78A` 근처)** 한 컬러만. hover/active 는 명도 한 단계 변형으로. 그라데이션 X.
- 모노 (Geist Mono) 는 코드 블록과 path 표기에만. 본문에 흐트러뜨리지 말 것.

## 6. 제약 / 비기능 요구

- **기능 회귀 절대 X** — 화면별 동작·단축키·SSE 진행률·query string 동기화·상태 보존 모두 유지.
- TanStack Query / Zustand / React Router 그대로. 컴포넌트 시그니처 가능한 한 보존.
- 다크모드 default 유지 + 라이트모드 동작.
- shadcn/ui 컴포넌트 베이스 유지 — token (CSS variable) 만 재디자인.
- Tailwind 클래스 in-JSX. CSS-in-JS 도입 X.
- 번들 사이즈 — initial JS ≤ 250 kB gzip 유지.
- 접근성 — 키보드 nav, focus ring, ARIA label, 충분한 대비 (WCAG AA).
- 반응형 — 데스크탑 위주지만 1024px / 768px 에서 깨지지 않게.
- 의존성 추가는 정말 필요할 때만. `framer-motion` 정도는 추가 가능 (motion 이 의미 있을 때). lottie 같은 건 X.

## 7. 산출물 (Deliverable)

다음을 단계적으로 진행합니다. 각 stage 끝에서 사용자에게 짧게 patch 형태로 제시하고 다음 stage 로 자동 이어갑니다 (사용자가 멈추라고 하지 않는 한).

### Stage 1 — 디자인 시스템 spec

- `web/src/index.css` 의 CSS variable 재정의 (라이트/다크) — §5 의 색/톤 적용.
- `web/tailwind.config.ts` 의 `extend` 갱신 — 색상/font/radius/spacing 토큰.
- typography scale 정의 (h1/h2/h3/h4/body/caption/mono) — 한글 Pretendard 와 영문 Geist 의 metric 차이를 반영한 line-height·letter-spacing.
- icon size 룰 (Lucide 사용 중), spacing 룰 (8px 그리드).
- `web/src/lib/design-tokens.md` (신규) — 토큰 의도와 사용 예 짧게.

### Stage 2 — 핵심 화면 redesign

`/sessions` 를 reference 화면으로 잡고 redesign:

- `Layout` (상단 nav 포함)
- `SessionList` + `SessionListItem`
- `SearchBar` (keyword/semantic 토글)
- `SessionFilters`, `TagEditor`, `FavoriteButton`
- 빈 상태/로딩 상태/에러 상태

### Stage 3 — 나머지 화면 일괄 적용

- `SessionDetailRoute` (`SessionHeader`, `MiniChart`, `RelatedSessions`, `NoteEditor`)
- `WikiRoute` (검색 + 3-mode 토글, 마크다운 본문 — Tailwind `prose` 또는 자체 prose 토큰)
- `DailyRoute`, `GraphRoute`, `CommandsRoute`
- 전역 `JobBanner`, `JobToastListener`, `HotkeyHelpDialog`

### Stage 4 — micro-polish

- focus ring, hover/active state 통일.
- motion (Tailwind transition 우선, 핵심 지점만 framer-motion).
- 빈 상태 / 첫 실행 onboarding 디테일.

## 8. 작업 방식

각 stage 시작 시:
1. 디자인 의도 (어떤 reference 톤을 차용했는지) 한 단락 설명.
2. 변경할 파일 + 변경 범위를 bullet 으로 미리 제시.
3. 코드 patch — diff 형태 또는 전체 파일 (사용자가 손쉽게 적용할 수 있게).
4. 다음 stage 로 자동 진행. 단 다음에 해당하면 멈추고 짧게 보고:
   - 기능 동작이 모호하거나 회귀 위험이 있을 때
   - 의존성 추가가 필요할 때 (e.g., framer-motion)
   - §5 외의 큰 디자인 분기 결정이 필요할 때

## 9. 코드 위치 인덱스

- `web/src/routes/*.tsx` — 페이지
- `web/src/components/*.tsx` — 공용/페이지 컴포넌트
- `web/src/components/ui/*.tsx` — shadcn/ui primitives
- `web/src/index.css` — global styles + CSS variables
- `web/tailwind.config.ts` — Tailwind tokens
- `web/src/lib/api.ts`, `web/src/hooks/*.ts` — 데이터 레이어 (**손대지 말 것**)

## 10. 화면별 텍스트 wireframe (스크린샷 대체)

스크린샷이 없는 상태로 작업합니다. 각 화면의 현재 구조는 다음과 같이 가정하세요:

### `/sessions`

```
┌─────────────────────────────────────────────────────────────┐
│ Layout nav: [seCall logo]  Sessions  Wiki  Daily  Graph  Commands     [theme] │
├──────────────────┬──────────────────────────────────────────┤
│ [Search...]      │                                          │
│ [keyword|semantic]│  (좌측에서 세션을 선택하세요)             │
│ Filters: tags    │                                          │
│ Filters: agents  │                                          │
│ ─────────────    │                                          │
│ ▢ session-1      │                                          │
│   project · 12t  │                                          │
│   summary line   │                                          │
│ ▢ session-2      │                                          │
│   …              │                                          │
│ (무한 스크롤)     │                                          │
└──────────────────┴──────────────────────────────────────────┘
```

### `/sessions/:id`

```
┌─────────────────────────────────────────────────────────────┐
│ nav                                                         │
├─────────────────────────────┬───────────────────────────────┤
│ ← back  agent · project · date · 12 turns · ★ · #tag #tag  │
│ ─────────────                                               │
│ Turn 1 — User                                               │
│   prompt 본문                                               │
│ Turn 2 — Assistant                                          │
│   response 본문                                             │
│ …                            │ Mini-chart (턴/토큰/duration)│
│                              │ Related sessions             │
│                              │ Notes editor                 │
└─────────────────────────────┴───────────────────────────────┘
```

### `/wiki` `/wiki/:project`

```
┌─────────────────────────────────────────────────────────────┐
│ nav                                                         │
├──────────────────┬──────────────────────────────────────────┤
│ [Search wiki...] │  # Project name                          │
│ [keyword|semantic│  (last modified 2026-05-06)              │
│  |hybrid]        │  ─────                                   │
│ ─────            │  마크다운 본문 (h1~h4, code, list, link, │
│ Projects         │  blockquote, table, mermaid optional)    │
│ ▢ secall          │                                          │
│ ▢ tunaflow        │                                          │
│ ▢ obsidian-secall │                                          │
└──────────────────┴──────────────────────────────────────────┘
```

### `/daily`, `/graph`, `/commands`

- `/daily` — 날짜 picker + 그날의 세션을 프로젝트별로 grouping + 자동 생성 일기 (마크다운).
- `/graph` — 전체 화면 react-flow 캔버스, 좌하단에 legend (node types + relation types), 우하단에 controls (fit / zoom / reset).
- `/commands` — 4개 카드 grid (Sync / Ingest / Wiki Update / Graph Rebuild) + 각 카드에 last-run 표시 + 옵션 다이얼로그 트리거 + 실행 중이면 진행률 inline.

### 전역 elements

- `JobBanner` — 화면 상단(또는 하단) 1줄, 진행 중 job 한 개를 표시. progress bar + 현재 phase + 취소 버튼. 완료 시 fade-out.
- `JobToastListener` — 우하단 toast (성공: 녹색 dot, 실패: 적색 dot) — radix-toast 베이스.
- `HotkeyHelpDialog` — modal, key-bindings 표.

---

## 11. 시작 메시지 (claude.ai 에 그대로 붙여넣기)

```
안녕 — 내가 만든 로컬 도구 secall 의 웹 UI(web/)를 다시 디자인해줘.
첨부한 web-redesign.md 한 장에 stack/route/통증/방향성/제약/산출물/작업 방식이 다 정리돼 있어.
나는 디자인 결정에 매번 개입하지 않을 테니 §5 의 방향성("Calm/Editorial, Linear-tone, off-white + single teal accent")을 기준으로 알아서 정해줘.
스크린샷은 없고 §10 의 텍스트 wireframe 으로 화면 구조를 잡았어.

Stage 1(디자인 시스템 spec)부터 시작해줘.
의도 설명 → 변경 파일 bullet → 코드 patch 순서로 한 stage 씩 끝내고 자동으로 다음 stage 로 넘어가도 돼.
다음에만 멈추고 물어봐줘:
- 기능 회귀 위험이 보일 때
- 의존성 추가가 필요할 때 (framer-motion 정도는 OK)
- §5 외의 큰 디자인 분기 결정이 필요할 때
```
