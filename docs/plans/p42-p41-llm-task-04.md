---
type: task
plan_slug: p42-p41-llm
task_id: 04
title: Settings UI 폴리싱
parallel_group: B
depends_on: []
status: pending
updated_at: 2026-05-09
---

# Task 04 — Settings UI 폴리싱

## Changed files

수정:
- `web/src/routes/SettingsRoute.tsx` — dirty state 추적, save 후 폼 리셋,
  `gemini_api_key` masked input 안내 modal trigger, inline validation, 헤더 "변경됨" 배지.
- `web/src/hooks/useConfig.ts` — `useConfigPatch` 의 `onSuccess` 가 mutate variables (section) 를
  caller 에 노출하도록 변경 (현재 toast 만 띄움). caller 에서 dirty state 리셋에 사용.

신규:
- `web/src/components/settings/MaskedKeyInfoModal.tsx` (신규) — `<Dialog>` 기반 modal.
  `gemini_api_key` 의 masked input 클릭 시 표시. `.env` 파일 편집 가이드 + 환경변수 이름
  (`SECALL_GEMINI_API_KEY`) 안내.
- `web/src/lib/validators.ts` (신규 또는 기존 파일에 추가) — model name 의 단순 validator
  (`/^[a-zA-Z0-9._:-]+$/`), URL validator (반드시 http(s):// 시작).

회귀 테스트:
- `web/src/routes/SettingsRoute.test.tsx` (신규) — vitest + @testing-library/react.
  (a) dirty state 표시 (b) save 후 리셋 (c) invalid model 입력 시 inline error (d) masked input click 시 modal open.
  기존 web 의 vitest 환경 (`web/vitest.config.ts` 또는 동등) 활용.

## Change description

### 1. Dirty state 추적

```tsx
const initialRef = useRef<{wiki, graph, log, embedding} | null>(null);
useEffect(() => {
  if (data && !initialRef.current) {
    initialRef.current = { wiki: data.wiki, graph: data.graph, log: data.log, embedding: data.embedding };
  }
}, [data]);

const isDirty = (section: SectionKey) => {
  if (!initialRef.current) return false;
  const current = section === "wiki" ? wikiForm : section === "graph" ? graphForm : section === "log" ? logForm : embeddingForm;
  return JSON.stringify(current) !== JSON.stringify(initialRef.current[section]);
};
```

헤더에:
```tsx
{isDirty(section) && <Badge variant="warning">변경됨</Badge>}
```

### 2. Save 후 리셋

`useConfigPatch` 의 `onSuccess` 에서 `qc.invalidateQueries(["config"])` 가
새 데이터를 fetch → `useEffect(()=>setForm(data.section), [data])` 가 form 갱신.
**추가**: `initialRef.current[section] = newData` 도 업데이트해서 dirty state false 로.

```tsx
const patch = useConfigPatch();
const handleSave = async (section: SectionKey, body: unknown) => {
  await patch.mutateAsync({ section, body });
  // mutate 성공 후 invalidate → useEffect 가 form 재설정
  // initialRef 도 업데이트
  if (initialRef.current && data) {
    initialRef.current[section] = data[section];
  }
};
```

### 3. Masked input 안내 modal

`gemini_api_key` 필드는 현재 disabled or readonly. 클릭 시:

```tsx
<MaskedKeyInfoModal
  open={modalOpen}
  envVar="SECALL_GEMINI_API_KEY"
  description="Gemini API key 는 환경변수 또는 .env 로만 설정할 수 있습니다."
/>
```

modal 본문:
- `.env` 위치 (`~/.config/secall/.env` 또는 project root)
- shell 명령 예시: `echo 'SECALL_GEMINI_API_KEY=...' >> ~/.config/secall/.env`
- 보안 안내 (chmod 600, git 제외)

### 4. Inline validation

각 model input 의 onChange:
```tsx
const isValid = /^[a-zA-Z0-9._:-]+$/.test(value);
```

invalid 시 input 아래에 `<p className="text-status-danger text-t-meta">잘못된 모델 이름</p>`,
저장 버튼 disabled.

URL field:
```tsx
const isValidUrl = (s: string) => {
  try { new URL(s); return s.startsWith("http://") || s.startsWith("https://"); }
  catch { return false; }
};
```

### 5. design-tokens 준수

새 컴포넌트 (`MaskedKeyInfoModal`, Badge) 는 P41 의 design-tokens 사용:
- `bg-[var(--surface)]`, `border-hairline`, `text-text-3`, `text-status-danger`
- 신규 색상 토큰 추가 X

### 6. 시각 회귀 방지

- TopNav / 다른 라우트 변경 X.
- 본 task 는 SettingsRoute 안에서만 변경.
- bundle size 영향 — modal 은 lazy import (`React.lazy`) 또는 inline (modal 작아서 inline OK).

## Dependencies

- 의존 task 없음 (P41 task 04 의 SettingsRoute 가 이미 존재).
- npm dep: 추가 없음. shadcn/ui 의 `Dialog` / `Badge` 사용 (이미 등록되어 있을 가능성).
  미등록 시 `pnpm dlx shadcn-ui@latest add dialog badge` 로 추가 — 본 task 의 verification 에서 lockfile diff 확인.

## Verification

```bash
cd web

# 1. type / lint
pnpm typecheck
pnpm lint --max-warnings 0

# 2. unit test
pnpm test -- SettingsRoute

# 3. build (bundle size 영향 확인)
pnpm build
# initial JS ≤ 250 kB gzip 유지 (P41 budget)

# 4. (수동) localhost
secall serve --port 8090 --allow-config-edit
# http://localhost:8090/settings:
#   - Wiki / Graph / Log / Embedding 카테고리 전환
#   - 값 변경 → 헤더에 "변경됨" 배지 표시
#   - 저장 → 토스트 + 배지 사라짐
#   - graph.gemini_api_key 클릭 → modal 표시
#   - wiki.haiku.model 에 "잘못 모델!@#" 입력 → inline error + 저장 비활성

# 5. read-only 모드 (without --allow-config-edit)
secall serve --port 8091
# /settings → form disabled + "읽기 전용" 안내 (P41 동작 유지)
```

## Risks

- **dirty state 의 reference equality** — `JSON.stringify` 비교는 key 순서에 민감.
  serialize 결과가 안정적이면 OK. 또는 deep-equal helper 사용.
- **modal 의 z-index** — TopNav 와 sticky content 위에 표시되도록 shadcn Dialog 의 default z-index 확인.
- **form schema 갱신** — backend 가 새 키 추가 시 form 자동 반영 X (수동 추가 필요). task 04 는 폴리싱 한정 — schema 변경 없음.
- **bundle size** — Dialog 컴포넌트 추가 시 +5-10 kB. `import("@/components/settings/MaskedKeyInfoModal")` lazy import 로 완화.
- **i18n** — UI 문구 한국어 hard-coded. 본 task 는 그 패턴 유지 — i18n 도입은 별도 plan.
- **vitest 환경** — `web/` 에 vitest 가 이미 설정돼 있는지 확인 후 진행. 없으면 본 task 에서
  최소 설정 추가 (`vitest`, `jsdom`, `@testing-library/react` dev-dep).

## Scope boundary (수정 금지)

- `crates/` — 본 task 는 web only.
- `web/src/routes/{Sessions,Wiki,Daily,Graph,Commands}Route.tsx` — 다른 라우트 변경 X.
- `web/src/components/TopNav.tsx` — 변경 X.
- `web/src/lib/api.ts` — `configGet` / `configPatch` 시그니처 변경 X (P41 task 04 영역).
- `web/src/lib/design-tokens.md` — 토큰 추가 / 갱신 X (필요 시 별도 plan).
