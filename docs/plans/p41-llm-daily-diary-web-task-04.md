---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 04
title: Web Settings 라우트 + form
parallel_group: C
depends_on: [03]
status: pending
updated_at: 2026-05-08
---

# Task 04 — Web Settings 라우트

## Changed files

수정:
- `web/src/routes/router.tsx` — `SettingsRoute` lazy import + `{ path: "settings", element: lazyEl(SettingsRoute) }` 추가.
- `web/src/components/TopNav.tsx` — 우상단 icons 영역에 톱니(Settings) 아이콘 추가, 클릭 시 `/settings` navigate. 단축키 hint 도 같이 (예: `g x`). NAV_ITEMS 에는 추가 안 함 (primary nav 는 5개 유지, settings 는 utility).
- `web/src/lib/api.ts` — 새 메서드: `configGet()` / `configPatch(section, body)`.
- `web/src/lib/store.ts` — 선택: 편집 중 dirty state 추적 (form 단위).

신규:
- `web/src/routes/SettingsRoute.tsx` (신규) — 카테고리 4개 (Wiki / Graph / Log / Embedding) 의 form. Tab 또는 vertical accordion.
- `web/src/components/settings/WikiSettingsForm.tsx`, `GraphSettingsForm.tsx`, `LogSettingsForm.tsx`, `EmbeddingSettingsForm.tsx` (신규) — 각 섹션 form. 또는 단일 SettingsRoute 안에 inline (분량 봐서).
- `web/src/hooks/useConfig.ts` (신규) — TanStack Query 의 `useQuery` 로 GET, `useMutation` 으로 PATCH. invalidate 후 refetch.

## Change description

### 1. `/settings` 라우트

URL: `/settings` (또는 `/settings/wiki` 같이 sub-route 로 분할도 가능 — 본 task 는 단일 라우트 + 좌측 카테고리 nav 추천).

레이아웃:
- 상단: TopNav (그대로)
- 좌측 sidebar (240px): 카테고리 list (Wiki / Graph / Log / Embedding) + 우하단에 "config 파일 경로 보기" 버튼
- 본문: 선택된 카테고리의 form

### 2. Wiki form

- `default_backend`: select (`claude` / `codex` / `haiku` / `ollama` / `lmstudio`)
- `review_model`: select (`sonnet` / `opus`)
- backend 별 sub-section (collapse):
  - claude: model (text)
  - codex: model
  - haiku: model + max_tokens
  - ollama: api_url + model + max_tokens
  - lmstudio: api_url + model + max_tokens

### 3. Graph form

- `semantic`: toggle
- `semantic_backend`: select (`ollama` / `anthropic` / `gemini` / `lmstudio` / `disabled`)
- `ollama_model` / `anthropic_model` / `gemini_model`: text
- `gemini_api_key`: **masked input** (placeholder `<env>` — 실제 값 표시 X, 환경변수 안내 link 만)

### 4. Log form

- `backend`: select (Wiki 와 동일 5개)
- `model` / `api_url` / `max_tokens` (해당 backend 가 필요한 것만)

### 5. Embedding form

- `backend`: select (`ollama` / `ort` / `openai` / `openvino`)
- 백엔드별 옵션 (ollama_url / ollama_model / model_path / openai_model / openvino_device)

### 6. UX 디테일

- Form 마운트 시 `useConfig()` 로 GET → 초기값 로드.
- 편집 시 dirty state 표시 (헤더에 "변경됨" 배지).
- 저장 버튼 클릭 → `configPatch(section, body)` mutation.
- 성공 → toast "설정 저장됨", form reset to clean.
- 실패 (e.g., `--allow-config-edit` 비활성) → toast "config 편집 비활성. `secall serve --allow-config-edit` 로 다시 시작하세요".
- 모든 input 은 design tokens 사용 (`bg-[var(--surface)]`, `border-border-soft`, `text-t-body`).

### 7. Read-only 모드

`useConfig()` 가 PATCH 비활성을 detect (시도 후 403 → readonly state). 그 경우 모든 input 을 disabled + 상단에 "읽기 전용 모드 — config.toml 직접 편집하세요" 안내.

## Dependencies

- **task 03 필수** — REST endpoint.
- npm dep: 추가 없음. shadcn/ui 의 Select / Input / Button / Switch / Tabs 사용.
- 기존 `useUi` store 패턴 따름. 신규 store 도입 X.

## Verification

```bash
cd web && pnpm typecheck       # 타입 확인
pnpm build                      # 번들 사이즈 한도 (initial JS ≤ 250 kB gzip)

# (수동) settings 페이지 동작
secall serve --port 8090 --allow-config-edit
# 브라우저: http://localhost:8090/settings
# - 4 카테고리 form 모두 마운트
# - 저장 후 config.toml 갱신 확인 (cat ~/Library/Application\ Support/secall/config.toml)
# - read-only: secall serve --port 8091 (without flag) → form disabled + 안내 노출
```

## Risks

- **Form 분량 + 일관성** — 4 form 이 비슷한 패턴이라 boilerplate 발생. 공통 hook (예: `useFormSection<T>`) 또는 단일 SettingsRoute 안에 inline 으로 합치는 것도 OK. 본 task 는 가독성 우선.
- **Backend 별 conditional field 표시** — wiki backend 마다 필요한 필드가 다름. UI 가 selected backend 의 sub-section 만 노출하도록.
- **API key masked input** — 사용자가 실제 값 입력하려 하면 안 됨. 명확한 안내 + disabled 또는 readonly. 클릭 시 `.env` 파일 편집 가이드 modal.
- **번들 사이즈** — 새 form 컴포넌트 4개 + react-hook-form (옵션). 현재 87 kB → 100 kB 안 넘게.
- **시각 회귀** — TopNav 의 icons 추가가 다른 라우트 시각에 영향 X (단순 1 아이콘 추가).

## Scope boundary (수정 금지)

- `web/src/routes/{Sessions,Wiki,Daily,Graph,Commands}Route.tsx` — 다른 라우트 시각 변경 X.
- `web/src/components/TopNav.tsx` — settings 아이콘 추가 외 변경 X.
- backend 영역 (`crates/secall-core/src/`) — task 03 영역.
- `crates/secall/src/commands/config.rs` — task 05 영역.
