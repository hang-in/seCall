# secall-web design tokens

**원본**: `docs/prompts/2026-05-06/web-redesign.md` 의 Stage 1 spec + Claude Design export bundle (`secall-web.zip`).
**톤**: Calm / Editorial · Linear-tone · indigo accent (단일).

## 토큰의 두 층

1. **prototype hex 토큰** — `--bg`, `--surface{,-2,-3}`, `--text{,-2,-3,-4}`, `--accent` 등.
   - 컴포넌트 JSX 에서 `bg-[var(--bg)]`, `text-text-2` (Tailwind 매핑됨), `bg-brand` 등으로 사용.
2. **shadcn/ui 호환 hsl 토큰** — `--background`, `--foreground`, `--primary`, `--card`, `--popover`, `--ring` 등.
   - radix-ui 베이스 컴포넌트 (Dialog/Popover/Toast/DropdownMenu) 가 직접 참조. 두 층이 같은 색을 가리키도록 매핑돼 있음.

## 사용 가이드 (실전)

| 의도 | Tailwind class |
|---|---|
| 페이지 배경 | `bg-[var(--bg)]` 또는 `bg-background` |
| 카드/패널 배경 | `bg-surface` (= `--surface`) |
| 헤더 hover, sub-panel | `bg-surface-2` |
| 강조되지 않은 헤더 | `bg-surface-3` |
| hairline 1px divider | `border-b border-hairline` |
| 일반 border | `border border-border` |
| 강조 border | `border border-border-strong` |
| 본문 | `text-text` |
| 보조 본문 | `text-text-2` |
| meta / caption | `text-text-3` |
| 비활성 / hint | `text-text-4` |
| 대표 액션 (버튼) | `bg-brand text-text-on-accent hover:bg-brand-hover` |
| 액션 hover bg | `bg-brand-soft` |
| 액션 outline | `border border-brand-border-soft text-brand` |
| 위험 (delete) | `text-status-danger` 또는 `bg-destructive text-destructive-foreground` |
| 성공 | `text-status-success` |
| 단축키 표기 | `<kbd>` (utility class `.kbd`) |
| eyebrow (uppercase) | `<span class="eyebrow">` |
| code 인라인 | `<span class="mono">` |

## Type scale

| 의도 | class | 크기/lh |
|---|---|---|
| 큰 타이틀 (페이지 H1 후보) | `text-t-display-s` | 22 / 30 |
| 섹션 타이틀 | `text-t-h1` | 18 / 26 |
| sub 타이틀 | `text-t-h2` | 15 / 22 |
| 작은 sub 타이틀 | `text-t-h3` | 14 / 20 |
| 본문 (default) | `text-t-body` | 14 / 22 |
| reading column (마크다운 본문) | `text-t-prose` | 15 / 26 |
| 작은 본문 | `text-t-small` | 13 / 19 |
| meta | `text-t-meta` | 12 / 16 |
| caption / eyebrow | `text-t-caption` | 11 / 14 |
| mono (path/code) | `text-t-mono` | 12.5 / 19 |

## Spacing — 8-grid (4 half-step)

`p-ds-2` (=8px), `gap-ds-3` (=12px), `mt-ds-6` (=24px) 등 `ds-N` 으로 통일.

| token | px |
|---|---|
| ds-1 | 4 |
| ds-2 | 8 |
| ds-3 | 12 |
| ds-4 | 16 |
| ds-5 | 20 |
| ds-6 | 24 |
| ds-7 | 32 |
| ds-8 | 40 |
| ds-9 | 56 |
| ds-10 | 72 |

레이아웃 토큰: `nav-h` (48px), `list-w` (376px), `read-w` (720px).

## Radius / Elevation / Motion

- 라운딩: `rounded-sm` (4) / `rounded-md` (6) / `rounded-lg` (8) / `rounded-xl` (10) / `rounded-2xl` (14) / `rounded-full`.
- 그림자: `shadow-ds-1` (subtle hairline), `shadow-ds-2` (card), `shadow-ds-pop` (modal/dropdown).
- transition: `duration-fast|base|slow` (120/160/240ms) + `ease-ds` (`cubic-bezier(.2,0,0,1)`).

## 라이트 vs 다크

- 다크모드 default. `<html class="dark">` 로 토글. 토글 UI 는 TopNav 우상단.
- 같은 토큰 이름이 라이트/다크에서 의미만 같을 뿐 hex 값은 다르게 정의됨.
- 다크모드의 텍스트는 순백 X — `#E6E7EA` (off-white) 로 eye fatigue 회피.

## Accent — indigo 단일

prototype 은 4-variant 토글 (teal/sage/amber/indigo) 였으나 production 은 **indigo 만**.

- 라이트: `#4258C3`, hover `#384BA9`, soft `rgba(66,88,195,.09)`
- 다크: `#6E84E2`, hover `#8B9CEB`, soft `rgba(110,132,226,.10)`

`bg-brand` / `text-brand` / `border-brand-border-soft` 로 사용.

## 폰트

- 한글 본문: Pretendard Variable (cdn `jsdelivr/orioncactus/pretendard@v1.3.9`)
- 영문 본문: Geist (Google Fonts)
- 코드 / path: Geist Mono

`font-sans` 가 Pretendard → Geist 폴백, `font-mono` 가 Geist Mono → ui-monospace 폴백.

## 회귀 검증 (Stage 1 적용 후)

- `pnpm typecheck` — 컴포넌트 토큰 변경으로 깨지면 fix
- 라이트/다크 모두 한 번씩 시각 점검 (Stage 2 부터 컴포넌트별 적용)
- shadcn 컴포넌트 (Dialog/Popover/Toast 등) 가 새 indigo accent 로 자연스럽게 흐르는지 확인
