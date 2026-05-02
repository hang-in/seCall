---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 04
parallel_group: A
depends_on: []
---

# Task 04 — 키보드 단축키 + `?` 도움말 다이얼로그

## Changed files

수정:
- `web/src/routes/Layout.tsx` — `useGlobalHotkeys()` 호출 + `<HotkeyHelpDialog />` 마운트
- `web/package.json` — `react-hotkeys-hook` 추가

신규:
- `web/src/hooks/useGlobalHotkeys.ts` — 전역 단축키 등록 (라우팅, 검색 포커스, 모드 토글 등)
- `web/src/hooks/useListHotkeys.ts` — 리스트 컨텍스트 단축키 (j/k 이동, Enter 선택, [/] prev/next)
- `web/src/components/HotkeyHelpDialog.tsx` — `?` 누르면 열리는 단축키 표
- `web/src/lib/hotkeyStore.ts` — Zustand store 또는 useUi 확장 — `helpDialogOpen` 추가

## Change description

### 1. 의존성 추가

```bash
cd web && pnpm add react-hotkeys-hook
```

### 2. `useUi` 확장

`web/src/lib/store.ts`에 추가:
```ts
interface UiState {
  // ...
  helpDialogOpen: boolean;
  toggleHelpDialog: () => void;
}
```

### 3. `useGlobalHotkeys`

```ts
import { useHotkeys } from "react-hotkeys-hook";
import { useNavigate } from "react-router";
import { useUi } from "@/lib/store";

export function useGlobalHotkeys() {
  const navigate = useNavigate();
  const toggleGraph = useUi((s) => s.toggleGraphOverlay);
  const toggleHelp = useUi((s) => s.toggleHelpDialog);

  // ?로 도움말
  useHotkeys("shift+/", () => toggleHelp(), { preventDefault: true });

  // / 검색 포커스
  useHotkeys("/", (e) => {
    e.preventDefault();
    const input = document.querySelector<HTMLInputElement>(
      'input[data-hotkey="search"]',
    );
    input?.focus();
  });

  // g d, g w, g s, g c — 라우트 이동 (chord)
  useHotkeys("g>d", () => navigate("/daily"));
  useHotkeys("g>w", () => navigate("/wiki"));
  useHotkeys("g>s", () => navigate("/sessions"));
  useHotkeys("g>c", () => navigate("/commands"));

  // G — 그래프 오버레이 토글
  useHotkeys("g g", () => toggleGraph());
}
```

> `react-hotkeys-hook`은 chord(`g>d`)를 지원. 입력 필드 안에서는 비활성 (기본).

### 4. `useListHotkeys`

SessionList 컴포넌트에서 호출 — 리스트 항목 j/k 이동:
```ts
export function useListHotkeys(items: { id: string }[], selectedId: string | undefined, onSelect: (id: string) => void) {
  const navigate = useNavigate();

  useHotkeys("j", () => {
    if (!items.length) return;
    const idx = items.findIndex(i => i.id === selectedId);
    const next = items[Math.min(idx + 1, items.length - 1)];
    if (next) onSelect(next.id);
  });
  useHotkeys("k", () => {
    if (!items.length) return;
    const idx = items.findIndex(i => i.id === selectedId);
    const prev = items[Math.max(idx - 1, 0)];
    if (prev) onSelect(prev.id);
  });
  // [ / ] — SessionDetail 진입한 상태에서 prev/next 세션 이동
  useHotkeys("[", () => { /* idx-1 navigate */ });
  useHotkeys("]", () => { /* idx+1 navigate */ });
}
```

### 5. SessionDetail 컨텍스트 단축키

SessionDetailRoute에서:
- `f` — favorite toggle (FavoriteButton의 onClick 호출)
- `e` — notes 편집기 포커스 (Task 09 의존이지만 본 task에서 hotkey만 등록)

```ts
useHotkeys("f", () => {
  const btn = document.querySelector<HTMLButtonElement>('[data-hotkey="favorite"]');
  btn?.click();
});
useHotkeys("e", () => {
  const editor = document.querySelector<HTMLTextAreaElement>('[data-hotkey="notes"]');
  editor?.focus();
});
```

`data-hotkey` 속성은 FavoriteButton/NoteEditor에 추가 (Task 06/09에서).

### 6. `HotkeyHelpDialog`

shadcn `Dialog` 사용. 단축키 표:
```tsx
const HOTKEYS = [
  { keys: "?", desc: "이 도움말 열기/닫기" },
  { keys: "/", desc: "검색 포커스" },
  { keys: "j / k", desc: "리스트 다음/이전 이동" },
  { keys: "[ / ]", desc: "세션 prev/next" },
  { keys: "Enter", desc: "선택" },
  { keys: "g d", desc: "Daily로 이동" },
  { keys: "g w", desc: "Wiki로 이동" },
  { keys: "g s", desc: "Sessions로 이동" },
  { keys: "g c", desc: "Commands로 이동" },
  { keys: "g g", desc: "그래프 오버레이 토글" },
  { keys: "f", desc: "현재 세션 즐겨찾기 토글" },
  { keys: "e", desc: "현재 세션 노트 편집" },
  { keys: "Esc", desc: "다이얼로그/오버레이 닫기" },
];
```

테이블 형태로 렌더. 다크 테마 유지.

### 7. Layout 통합

```tsx
useGlobalHotkeys();
return (
  <div>
    ...
    <HotkeyHelpDialog />
    ...
  </div>
);
```

## Dependencies

- 외부 npm: `react-hotkeys-hook`
- 내부 task: 없음 (Task 09의 NoteEditor가 `data-hotkey="notes"` 속성을 가져야 `e` 단축키 의미가 살지만, 본 task는 hotkey만 등록 — 속성 없으면 no-op)

## Verification

```bash
cd web && pnpm add react-hotkeys-hook
cd web && pnpm typecheck && pnpm build
cargo check --all-targets

# 수동:
# 어떤 페이지에서든 ?로 도움말 열림
# / 누르면 SearchBar 포커스
# g s, g d, g w, g c 라우트 이동
# /sessions에서 j/k로 리스트 이동, Enter로 선택
```

## Risks

- **chord (g>d) 입력 시간**: 기본 1초 timeout. 사용자가 g 누른 뒤 1초 안에 다음 키 안 누르면 cancel
- **input 안 hotkey**: react-hotkeys-hook 기본은 input 안에서 비활성화. ?도움말 표시 키는 textarea/input에서도 동작 원하면 `enableOnFormTags: ["INPUT", "TEXTAREA"]` 옵션
- **충돌**: 다른 라이브러리 (xyflow 등)도 keyboard 이벤트 사용 가능. graph 오버레이 열린 상태에서 ESC만 처리, j/k 등은 이벤트 전파로 둠
- **`data-hotkey` 속성**: FavoriteButton/NoteEditor가 해당 속성을 가져야 함. 본 task에서 명시적으로 추가 — Task 06/09에서 별도 hotkey 등록 필요 없음 (DOM query 방식)

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`, `.github/`, `README*`
- `web/src/components/{Session*,TagEditor,Favorite*,Date*,Markdown*,Job*,Graph*,Command*}.tsx` 본체 — 단 `data-hotkey` 속성 추가는 본 task 허용 (FavoriteButton에 한 줄)
- `web/src/routes/{Sessions,SessionDetail,Daily,Wiki,Commands}Route.tsx` 본체 — useListHotkeys/SessionDetail hotkey 호출 한 줄만 본 task 허용
