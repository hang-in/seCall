---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 06
parallel_group: E
depends_on: [02, 05]
---

# Task 06 — 일일 일기 + 위키 + 태그/즐겨찾기 UI

## Changed files

수정:
- `web/src/routes/DailyRoute.tsx` — 일일 일기 본격 구현 (날짜 네비, 마크다운 렌더)
- `web/src/routes/WikiRoute.tsx` — 위키 본격 구현 (프로젝트 리스트 + 마크다운)
- `web/src/routes/SessionDetailRoute.tsx` — 헤더에 태그 편집 + 즐겨찾기 토글 추가
- `web/src/routes/router.tsx` — 라우트는 이미 등록됨, 변경 없음

신규:
- `web/src/components/SessionHeader.tsx` — 세션 메타 + 태그 편집 + 즐겨찾기 토글
- `web/src/components/TagEditor.tsx` — 태그 칩 + 추가 입력 + 자동완성
- `web/src/components/FavoriteButton.tsx` — 별표 토글 (낙관적 업데이트)
- `web/src/components/DateNavigator.tsx` — 일일 일기 날짜 prev/next/today
- `web/src/hooks/useDaily.ts`, `web/src/hooks/useWiki.ts` — TanStack Query 훅
- `web/src/hooks/useTagMutations.ts` — setTags / setFavorite mutation
- `web/src/lib/allTags.ts` — 전체 세션의 태그 모음 (자동완성용, 클라이언트에서 집계)

## Change description

### 1. SessionHeader

`web/src/components/SessionHeader.tsx`:
- 첫 줄: 세션 ID (단축 6자) + agent + project + 모델
- 둘째 줄: 시작 시간 + turn 수 + (옵션) tools_used
- 우상단: FavoriteButton + (Phase 1 예고: "원본 위치" 링크)
- 셋째 줄: TagEditor (태그 칩 + 추가)

```tsx
import { Star } from "lucide-react";
import { TagEditor } from "./TagEditor";
import { FavoriteButton } from "./FavoriteButton";

export function SessionHeader({ session }: { session: SessionMeta }) {
  return (
    <header className="border-b border-border pb-4 mb-6 space-y-3">
      <div className="flex items-start justify-between">
        <div>
          <div className="text-xs text-muted-foreground">{session.id.slice(0, 8)}</div>
          <h1 className="text-xl font-semibold mt-1">
            {session.project ?? session.agent}
          </h1>
          <div className="text-xs text-muted-foreground mt-1">
            {session.start_time} · {session.turn_count} turns · {session.agent}
            {session.model ? ` · ${session.model}` : ""}
          </div>
        </div>
        <FavoriteButton sessionId={session.id} initial={session.is_favorite} />
      </div>
      <TagEditor sessionId={session.id} initial={session.tags} />
    </header>
  );
}
```

### 2. TagEditor

`web/src/components/TagEditor.tsx`:
- 현재 태그를 색상 칩으로 표시 (tagColor 사용), 각 칩 우측에 X 버튼 (삭제)
- 입력 필드 + Enter 추가 / 쉼표로도 추가 가능
- 자동완성: 모든 세션의 태그 합집합에서 prefix 매칭 (간단)
- 삭제/추가 시 `setTags` mutation 호출 (서버 정규화 결과로 갱신)

```tsx
import { useState } from "react";
import { X, Plus } from "lucide-react";
import { tagColor } from "@/lib/tagColor";
import { useSetTags } from "@/hooks/useTagMutations";
import { useAllTags } from "@/lib/allTags";

interface Props {
  sessionId: string;
  initial: string[];
}

export function TagEditor({ sessionId, initial }: Props) {
  const [tags, setTags] = useState(initial);
  const [draft, setDraft] = useState("");
  const mutation = useSetTags(sessionId);
  const allTags = useAllTags();

  const commit = async (next: string[]) => {
    setTags(next);
    const res = await mutation.mutateAsync(next);
    setTags(res.tags);  // 서버 정규화 결과
  };

  const add = () => {
    const v = draft.trim();
    if (!v) return;
    const next = Array.from(new Set([...tags, v]));
    commit(next);
    setDraft("");
  };

  const remove = (t: string) => commit(tags.filter((x) => x !== t));

  const suggestions = draft
    ? allTags.filter((t) => t.startsWith(draft.toLowerCase()) && !tags.includes(t)).slice(0, 5)
    : [];

  return (
    <div className="flex flex-wrap items-center gap-2">
      {tags.map((t) => (
        <span key={t} className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ring-1 ${tagColor(t)}`}>
          {t}
          <button onClick={() => remove(t)} aria-label={`remove tag ${t}`}>
            <X className="size-3 opacity-60 hover:opacity-100" />
          </button>
        </span>
      ))}
      <div className="relative">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              add();
            }
          }}
          placeholder="+ tag"
          className="bg-transparent border border-border rounded px-2 py-0.5 text-xs w-24 focus:w-40 transition-all outline-none focus:ring-1 focus:ring-ring"
        />
        {suggestions.length > 0 && (
          <div className="absolute top-full left-0 mt-1 bg-card border border-border rounded shadow-lg z-10 min-w-32">
            {suggestions.map((s) => (
              <button
                key={s}
                onClick={() => { setDraft(""); commit(Array.from(new Set([...tags, s]))); }}
                className="block w-full text-left px-2 py-1 text-xs hover:bg-accent"
              >
                {s}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
```

### 3. FavoriteButton

`web/src/components/FavoriteButton.tsx`:
```tsx
import { Star } from "lucide-react";
import { useState } from "react";
import { useSetFavorite } from "@/hooks/useTagMutations";

export function FavoriteButton({ sessionId, initial }: { sessionId: string; initial: boolean }) {
  const [on, setOn] = useState(initial);
  const mutation = useSetFavorite(sessionId);
  return (
    <button
      onClick={() => {
        const next = !on;
        setOn(next);  // 낙관적
        mutation.mutate(next, { onError: () => setOn(on) });
      }}
      aria-label={on ? "즐겨찾기 해제" : "즐겨찾기"}
      className="p-2 rounded hover:bg-accent"
    >
      <Star className={`size-5 ${on ? "fill-amber-400 text-amber-400" : "text-muted-foreground"}`} />
    </button>
  );
}
```

### 4. Mutation 훅

`web/src/hooks/useTagMutations.ts`:
```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";

export function useSetTags(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (tags: string[]) => api.setTags(sessionId, tags),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sessions"] });
      qc.invalidateQueries({ queryKey: ["allTags"] });
    },
  });
}

export function useSetFavorite(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (favorite: boolean) => api.setFavorite(sessionId, favorite),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sessions"] }),
  });
}
```

### 5. allTags 훅

`web/src/lib/allTags.ts`:
```ts
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export function useAllTags(): string[] {
  // 단순: 첫 페이지 100개 세션의 태그 합집합
  // 정밀하면 별도 /api/tags 엔드포인트 추가 (Phase 1)
  const { data } = useQuery({
    queryKey: ["allTags"],
    queryFn: () => api.listSessions({ page: 1, page_size: 100 }),
  });
  if (!data) return [];
  const set = new Set<string>();
  data.items.forEach((s) => s.tags.forEach((t) => set.add(t)));
  return Array.from(set).sort();
}
```

### 6. DailyRoute

`web/src/routes/DailyRoute.tsx`:
```tsx
import { useNavigate, useParams } from "react-router";
import { format, parseISO, addDays, subDays } from "date-fns";
import { useDaily } from "@/hooks/useDaily";
import { MarkdownView } from "@/components/MarkdownView";
import { Button } from "@/components/ui/button";
import { ChevronLeft, ChevronRight, Calendar } from "lucide-react";

export default function DailyRoute() {
  const { date } = useParams();
  const navigate = useNavigate();
  const today = format(new Date(), "yyyy-MM-dd");
  const current = date ?? today;
  const { data, isLoading } = useDaily(current);

  const go = (d: string) => navigate(`/daily/${d}`);

  return (
    <div className="grid grid-cols-[320px_1fr] h-full">
      <div className="border-r border-border p-4 space-y-3">
        <div className="flex items-center gap-2">
          <Button size="icon" variant="ghost" onClick={() => go(format(subDays(parseISO(current), 1), "yyyy-MM-dd"))}>
            <ChevronLeft className="size-4" />
          </Button>
          <input
            type="date"
            value={current}
            onChange={(e) => go(e.target.value)}
            className="bg-transparent border border-border rounded px-2 py-1 text-sm flex-1"
          />
          <Button size="icon" variant="ghost" onClick={() => go(format(addDays(parseISO(current), 1), "yyyy-MM-dd"))}>
            <ChevronRight className="size-4" />
          </Button>
        </div>
        <Button variant="outline" size="sm" className="w-full" onClick={() => go(today)}>
          <Calendar className="size-3 mr-2" /> 오늘
        </Button>
      </div>
      <div className="overflow-auto p-6 max-w-4xl">
        {isLoading ? <div className="text-muted-foreground">Loading…</div> : <MarkdownView content={extractMarkdown(data)} />}
      </div>
    </div>
  );
}

function extractMarkdown(data: unknown): string {
  // /api/daily 응답 구조에 맞춰 markdown 추출
  // do_daily()는 일기 내용을 markdown으로 반환한다고 가정
  if (!data) return "";
  if (typeof data === "object" && data !== null && "markdown" in data) return String((data as any).markdown);
  return JSON.stringify(data, null, 2);  // 폴백
}
```

> `/api/daily` 정확한 응답 구조 확인 필요 (`do_daily()`). 본 task는 `{ markdown: "..." }` 가정. 실제로는 다를 수 있음 — 구현 시 `tool-request:rawq:do_daily` 또는 직접 Read.

`useDaily`:
```ts
// web/src/hooks/useDaily.ts
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export function useDaily(date: string) {
  return useQuery({
    queryKey: ["daily", date],
    queryFn: () => api.daily(date),
  });
}
```

`date-fns` 추가:
```bash
cd web && pnpm add date-fns
```

### 7. WikiRoute

`web/src/routes/WikiRoute.tsx`:
```tsx
import { useNavigate, useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { MarkdownView } from "@/components/MarkdownView";

export default function WikiRoute() {
  const { project } = useParams();
  const navigate = useNavigate();

  // 프로젝트 리스트 — listProjects 사용
  const { data: projects } = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });

  // 위키 검색은 /api/wiki (POST). 프로젝트별 페이지가 따로 있다면 do_wiki_search로 탐색
  const { data: wiki, isLoading } = useQuery({
    queryKey: ["wiki", project],
    queryFn: () => api.wikiSearch({ query: project ?? "", limit: 1 }),
    enabled: !!project,
  });

  return (
    <div className="grid grid-cols-[260px_1fr] h-full">
      <aside className="border-r border-border overflow-auto">
        <div className="p-3 text-xs text-muted-foreground uppercase tracking-wide">Projects</div>
        <div className="divide-y divide-border">
          {projects?.projects.map((p) => (
            <button
              key={p}
              onClick={() => navigate(`/wiki/${encodeURIComponent(p)}`)}
              className={`block w-full text-left px-3 py-2 text-sm hover:bg-accent ${p === project ? "bg-accent" : ""}`}
            >
              {p}
            </button>
          ))}
        </div>
      </aside>
      <div className="overflow-auto p-6 max-w-4xl">
        {!project ? (
          <div className="text-muted-foreground text-sm">좌측에서 프로젝트를 선택하세요</div>
        ) : isLoading ? (
          <div className="text-muted-foreground">Loading…</div>
        ) : (
          <MarkdownView content={extractWikiMarkdown(wiki)} />
        )}
      </div>
    </div>
  );
}

function extractWikiMarkdown(data: unknown): string {
  // /api/wiki 응답 구조에 맞춰 추출 — 본 task 구현 시 do_wiki_search 응답 확인
  if (!data) return "";
  // 가정: { results: [{ path, content, ... }] }
  if (typeof data === "object" && data !== null && "results" in data) {
    const arr = (data as any).results;
    if (Array.isArray(arr) && arr[0]?.content) return String(arr[0].content);
  }
  return "위키 내용을 찾을 수 없습니다";
}
```

> `do_wiki_search` 응답 확인 필요. 만약 wiki 본문을 직접 fetch해야 한다면 신규 엔드포인트 `GET /api/wiki/:project` 추가 검토 (Phase 1 또는 Task 03 추가). 본 task는 기존 wiki search 결과의 content 필드 사용 가정.

### 8. SessionDetailRoute 갱신

기존 placeholder 대신 SessionHeader 사용:
```tsx
import { useParams } from "react-router";
import { useSession } from "@/hooks/useSessions";
import { SessionHeader } from "@/components/SessionHeader";
import { MarkdownView } from "@/components/MarkdownView";

export default function SessionDetailRoute() {
  const { id } = useParams();
  const { data, isLoading, error } = useSession(id, true);

  if (!id) return null;
  if (isLoading) return <div className="p-6 text-muted-foreground">Loading…</div>;
  if (error) return <div className="p-6 text-rose-400">{error.message}</div>;
  if (!data) return null;

  // /api/get 응답 구조 확인 필요. 가정: { session: SessionMeta, body: string }
  const session = (data as any).session ?? data;
  const body = (data as any).body ?? (data as any).markdown ?? "";

  return (
    <div className="p-6 max-w-4xl">
      <SessionHeader session={session} />
      <MarkdownView content={body} />
    </div>
  );
}
```

## Dependencies

- Task 03 완료 (`/api/projects`, `/api/sessions`, PATCH 엔드포인트)
- Task 06 완료 (Layout, SessionsRoute, MarkdownView, hooks/useSessions)
- 신규 npm 패키지: `date-fns`

## Verification

```bash
# 1. 의존성 설치
cd web && pnpm add date-fns

# 2. 타입 체크
cd web && pnpm typecheck

# 3. 빌드 성공
cd web && pnpm build

# 4. # Manual: 통합 검증
#   - `cargo run -- serve --port 8080` + `cd web && pnpm dev`
#   - http://127.0.0.1:5173/sessions/<실제 ID>
#     - 헤더에 즐겨찾기 별표 표시. 클릭 시 채워짐 (낙관적). 페이지 새로고침 후에도 유지
#     - 태그 입력 필드에 "Rust" 입력 후 Enter — 칩으로 변환되고 "rust"로 정규화됨
#     - 칩 X 버튼으로 삭제 동작
#     - 자동완성: 다른 세션에 등록된 태그가 prefix 매칭으로 노출
#   - http://127.0.0.1:5173/daily
#     - 오늘 날짜로 자동 이동, 일기 내용 마크다운 렌더
#     - prev/next 버튼으로 날짜 이동
#     - date input으로 직접 선택
#   - http://127.0.0.1:5173/wiki
#     - 좌측에 프로젝트 리스트
#     - 프로젝트 클릭 시 우측에 위키 페이지 마크다운 렌더

# 5. Rust 측 영향 없음
cargo check --all-targets --all-features
```

## Risks

- **`/api/get`, `/api/daily`, `/api/wiki` 응답 형태 미확정**: 코드에 `extractMarkdown` 등 폴백 함수 작성. 구현 시 실제 응답 확인 후 정확한 타입으로 교체. 만약 wiki 본문이 응답에 포함 안 되면 `GET /api/wiki/:project` 신규 엔드포인트 추가 (Task 03 또는 본 task에서 추가)
- **자동완성 데이터 부족**: `useAllTags`가 첫 100개 세션만 봄. 더 정확하려면 `/api/tags` 신규 엔드포인트. MVP는 부족해도 동작
- **낙관적 업데이트와 invalidate 충돌**: setFavorite 직후 invalidate가 GET을 다시 호출. 응답이 늦으면 잠시 깜빡임. acceptable
- **태그 정규화 표시 차이**: 사용자가 "Rust" 입력했는데 칩에 "rust"로 표시 — 의도된 동작이지만 혼란 가능. UI에 짧은 안내 ("태그는 소문자로 저장됩니다") 또는 toast로 알림
- **date-fns 번들 크기**: tree-shaking 잘 됨 — `format`/`parseISO` 등 import한 함수만 포함
- **datepicker 네이티브**: `<input type="date">`는 브라우저별 UI 차이. Phase 1에서 shadcn DatePicker 도입

## Scope boundary

수정 금지:
- `crates/` 전체 — 백엔드 변경 금지 (단, `/api/wiki/:project` 신규 추가 필요 시 별도 협의)
- `web/src/routes/Layout.tsx` 그래프 토글 자체는 Task 05에서 추가됨, 본 task에서는 손대지 않음
- 그래프 오버레이 UI — Task 08
- `.github/workflows/`, `README.md` — Task 09
