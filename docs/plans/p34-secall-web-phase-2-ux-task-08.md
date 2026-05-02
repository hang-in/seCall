---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 08
parallel_group: B
depends_on: [00]
---

# Task 08 — 세션 노트 편집 UI (autosave 1s debounce)

## Changed files

수정:
- `web/src/lib/api.ts` — `setNotes(id, notes): Promise<{session_id, notes}>` 추가
- `web/src/hooks/useTagMutations.ts` — `useSetNotes(sessionId)` 추가 (또는 별도 훅으로)
- `web/src/components/SessionHeader.tsx` — 헤더 하단 또는 별도 패널에 `<NoteEditor sessionId={id} initial={detail.notes} />` 마운트

신규:
- `web/src/components/NoteEditor.tsx` — autosave 1s debounce + 저장 상태 표시 + `data-hotkey="notes"` 속성 (Task 05 키바인딩 연결)
- `web/src/hooks/useDebounce.ts` — generic debounce 훅 (없으면 신규)

## Change description

### 1. API + 훅

```ts
// api.ts
setNotes: (id: string, notes: string | null) =>
  jfetch<{ session_id: string; notes: string | null }>(
    `/api/sessions/${encodeURIComponent(id)}/notes`,
    { method: "PATCH", body: JSON.stringify({ notes }) },
  ),

// useTagMutations.ts
export function useSetNotes(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (notes: string | null) => api.setNotes(sessionId, notes),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sessions", "detail", sessionId] });
    },
  });
}
```

### 2. `useDebounce`

```ts
import { useEffect, useState } from "react";

export function useDebounce<T>(value: T, delay = 1000): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(t);
  }, [value, delay]);
  return debounced;
}
```

### 3. `NoteEditor.tsx`

```tsx
import { useEffect, useRef, useState } from "react";
import { Check, CircleDashed, Loader2 } from "lucide-react";
import { useDebounce } from "@/hooks/useDebounce";
import { useSetNotes } from "@/hooks/useTagMutations";

interface Props {
  sessionId: string;
  initial: string | null | undefined;
}

type SaveState = "idle" | "dirty" | "saving" | "saved" | "error";

export function NoteEditor({ sessionId, initial }: Props) {
  const [text, setText] = useState(initial ?? "");
  const debounced = useDebounce(text, 1000);
  const [state, setState] = useState<SaveState>("idle");
  const mutation = useSetNotes(sessionId);
  const lastSaved = useRef(initial ?? "");

  // 다른 세션으로 이동 시 prop 변화에 동기화
  useEffect(() => {
    setText(initial ?? "");
    lastSaved.current = initial ?? "";
    setState("idle");
  }, [sessionId, initial]);

  // 사용자가 입력하는 즉시 dirty 상태로
  useEffect(() => {
    if (text !== lastSaved.current) setState("dirty");
  }, [text]);

  // debounced 값이 바뀌고 lastSaved와 다르면 저장
  useEffect(() => {
    if (debounced === lastSaved.current) return;
    setState("saving");
    mutation.mutate(debounced || null, {
      onSuccess: () => {
        lastSaved.current = debounced;
        setState("saved");
      },
      onError: () => setState("error"),
    });
  }, [debounced]);

  return (
    <details className="border-t border-border pt-3 mt-3" data-hotkey-anchor="notes">
      <summary className="cursor-pointer text-sm font-medium flex items-center gap-2">
        노트
        <SaveIndicator state={state} />
      </summary>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="이 세션에 대한 메모..."
        rows={4}
        data-hotkey="notes"
        className="mt-2 w-full bg-background border border-border rounded p-2 text-sm font-mono resize-y focus:outline-none focus:ring-1 focus:ring-ring"
      />
    </details>
  );
}

function SaveIndicator({ state }: { state: SaveState }) {
  if (state === "saving") return <span className="text-xs text-muted-foreground inline-flex items-center gap-1"><Loader2 className="size-3 animate-spin" />저장 중</span>;
  if (state === "saved") return <span className="text-xs text-emerald-400 inline-flex items-center gap-1"><Check className="size-3" />저장됨</span>;
  if (state === "dirty") return <span className="text-xs text-amber-400 inline-flex items-center gap-1"><CircleDashed className="size-3" />변경됨</span>;
  if (state === "error") return <span className="text-xs text-rose-400">저장 실패</span>;
  return null;
}
```

### 4. SessionHeader 통합

```tsx
import { NoteEditor } from "./NoteEditor";

// SessionHeader 내부, 태그 에디터 아래 또는 별도 섹션
<NoteEditor sessionId={sessionId} initial={detail.notes} />
```

또는 SessionDetailRoute에서 SessionHeader 외부에 마운트 (디자인 결정). 본 task는 SessionHeader 내부에 `<details>`로 접혀 마운트.

### 5. Task 05 키 단축 (`e`)

Task 05의 `useGlobalHotkeys`가 `data-hotkey="notes"` selector로 textarea 포커스. NoteEditor textarea에 해당 속성 추가됨 → `e` 키 누르면 details 펼치기 + textarea 포커스. details 자동 펼침은 별도 코드:
```tsx
useHotkeys("e", () => {
  const anchor = document.querySelector<HTMLDetailsElement>('[data-hotkey-anchor="notes"]');
  if (anchor && !anchor.open) anchor.open = true;
  setTimeout(() => {
    document.querySelector<HTMLTextAreaElement>('[data-hotkey="notes"]')?.focus();
  }, 50);
});
```

이 hotkey는 Task 05에서 등록. 본 task는 `data-hotkey-anchor="notes"` + `data-hotkey="notes"` 속성만 제공.

## Dependencies

- 외부 npm: 없음
- 내부 task: Task 01 완료 (notes 컬럼 + PATCH 엔드포인트)

## Verification

```bash
cd web && pnpm typecheck && pnpm build
cargo check --all-targets

# 수동:
# /sessions/<id>에서 헤더 안 "노트" details 펼침 → textarea 입력
# 1초 후 "저장 중 → 저장됨" 표시
# 다른 세션 이동 후 다시 돌아오면 입력 내용 보존
# e 단축키로 textarea 포커스 (Task 05 통합 시)
```

## Risks

- **autosave 충돌**: 사용자가 빠르게 타이핑하면 매 1초마다 PATCH 호출. tag invalidate가 매번 → 깜빡임. invalidate 범위를 detail만으로 좁힘 (`["sessions", "detail", id]`)
- **debounce + 즉시 저장**: 사용자가 페이지 이탈 시 마지막 1초 미저장. `beforeunload` 이벤트 또는 explicit save 버튼 추가는 Phase 3+
- **빈 문자열 vs null**: 사용자가 텍스트 다 지우면 빈 문자열 저장. 백엔드는 ""와 null 모두 받아 저장. 본 UI는 `text.trim() === "" ? null : text`로 정리하지 않고 raw 저장 (사용자 의도 보존)
- **충돌**: 두 탭에서 같은 세션 노트 편집 시 last-write-wins. 사전 토론 결정 — 별도 lock 없음
- **details open state**: details 열림/닫힘 상태는 페이지 이동 시 리셋됨. 영구 저장은 Phase 3+
- **textarea contentEditable conflict**: 별도 충돌 없음

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`, `.github/`, `README*`
- `web/src/components/{SearchBar,SessionFilters,SessionList*,TagEditor,Favorite*,Date*,Markdown*,Job*,Graph*,Command*,Hotkey*,RelatedSessions,MiniChart}.tsx` (단 SessionHeader는 본 task — NoteEditor 마운트 한 줄)
- `web/src/routes/`
- `web/src/hooks/{useSessions,useDaily,useWiki,useJob*,useGraph,useGlobalHotkeys,useListHotkeys,useRelated}.ts` (단 useTagMutations 추가는 본 task)
- `web/src/lib/{api,types,store,allTags,tagColor,utils,queryClient,graphStyle,graphStartNode,highlight,hotkeyStore}.ts` (단 api.ts에 setNotes 추가는 본 task)
