---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 05
parallel_group: D
depends_on: [02, 04]
---

# Task 05 — 2-pane 레이아웃 + 검색/세션 뷰 구현

## Changed files

수정:
- `web/src/routes/Layout.tsx` — 2-pane 구조로 확장 (좌: 리스트, 우: 상세 outlet)
- `web/src/routes/SessionsRoute.tsx` — 검색 + 세션 리스트 본격 구현
- `web/src/routes/SessionDetailRoute.tsx` — 세션 상세 마크다운 렌더

신규:
- `web/src/components/SessionListItem.tsx` — 리스트 항목 컴포넌트 (제목/요약/태그/날짜)
- `web/src/components/SessionFilters.tsx` — 프로젝트/에이전트/날짜/즐겨찾기 필터 바
- `web/src/components/SearchBar.tsx` — 검색 입력 + recall 모드 토글 (keyword/semantic)
- `web/src/components/MarkdownView.tsx` — react-markdown + remark-gfm + 코드 하이라이트 (간단)
- `web/src/hooks/useSessions.ts` — TanStack Query 훅 (`useSessionsList`, `useSession`, `useSessionsRecall`)
- `web/src/lib/types.ts` — 공유 타입 정의 (api.ts에서 분리)

## Change description

### 1. Layout 2-pane 구조

`web/src/routes/Layout.tsx` 수정:
```tsx
import { NavLink, Outlet, useMatch } from "react-router";
import { Search, Calendar, Network, BookOpen, Star, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useUi } from "@/lib/store";

const NAV = [
  { to: "/sessions", icon: Search, label: "Sessions" },
  { to: "/daily", icon: Calendar, label: "Daily" },
  { to: "/wiki", icon: BookOpen, label: "Wiki" },
];

export default function Layout() {
  const toggleGraph = useUi((s) => s.toggleGraphOverlay);

  return (
    <div className="flex h-screen bg-background text-foreground">
      {/* Left navigation */}
      <aside className="w-56 shrink-0 border-r border-border p-4 space-y-1 flex flex-col">
        <div className="text-lg font-semibold mb-6 px-3">seCall</div>
        {NAV.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-2 px-3 py-2 rounded-md text-sm ${
                isActive
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-accent/50 text-muted-foreground"
              }`
            }
          >
            <Icon className="size-4" /> {label}
          </NavLink>
        ))}
        <div className="flex-1" />
        <Button variant="ghost" size="sm" onClick={toggleGraph} className="justify-start">
          <Network className="size-4 mr-2" /> Graph
        </Button>
      </aside>

      {/* Main 2-pane outlet — child routes render the panes */}
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
```

> 그래프 오버레이 자체는 Task 08에서 Layout 또는 별도 portal로 마운트.

### 2. SessionsRoute (좌측 리스트 + 우측 outlet)

`web/src/routes/SessionsRoute.tsx`:
```tsx
import { Outlet, useParams } from "react-router";
import { useState } from "react";
import { SearchBar } from "@/components/SearchBar";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionList } from "@/components/SessionList";

export default function SessionsRoute() {
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<SessionFilterState>({});

  return (
    <div className="grid grid-cols-[420px_1fr] h-full">
      {/* Left pane — search + filter + list */}
      <div className="border-r border-border flex flex-col overflow-hidden">
        <div className="p-3 border-b border-border space-y-2">
          <SearchBar value={query} onChange={setQuery} />
          <SessionFilters value={filters} onChange={setFilters} />
        </div>
        <div className="flex-1 overflow-auto">
          <SessionList query={query} filters={filters} />
        </div>
      </div>

      {/* Right pane — session detail outlet */}
      <div className="overflow-auto">
        <Outlet />
      </div>
    </div>
  );
}
```

라우트 트리 수정 (`web/src/routes/router.tsx`):
```tsx
{
  path: "sessions",
  element: <SessionsRoute />,
  children: [
    { index: true, element: <SessionEmptyState /> },
    { path: ":id", element: <SessionDetailRoute /> },
  ],
},
```

> 좌측 리스트는 항상 보임. 우측은 `index` (선택 안 됨 안내) 또는 `:id` (상세).

### 3. SearchBar 컴포넌트

`web/src/components/SearchBar.tsx`:
- shadcn `Input` + 좌측 search icon
- 검색 모드 토글 (keyword/semantic) — 작은 토글 버튼 또는 드롭다운
- 디바운스 300ms (lodash 없이 setTimeout으로)
- onChange는 디바운스된 값 전달

### 4. SessionFilters 컴포넌트

`web/src/components/SessionFilters.tsx`:
- 프로젝트 select (`api.listProjects()` 결과)
- 에이전트 select (`api.listAgents()` 결과)
- 날짜 범위 (date_from/date_to)
- 즐겨찾기 only 체크박스
- "초기화" 버튼

각 select는 shadcn에 없으므로 native `<select>` + Tailwind 스타일링 (간단). 또는 추가로 `pnpm dlx shadcn@latest add select` 실행.

### 5. SessionList 컴포넌트

`web/src/components/SessionList.tsx`:
```tsx
import { useNavigate, useParams } from "react-router";
import { useSessionsList } from "@/hooks/useSessions";
import { SessionListItem } from "./SessionListItem";
import { Loader2 } from "lucide-react";

interface Props {
  query: string;
  filters: SessionFilterState;
}

export function SessionList({ query, filters }: Props) {
  const { id } = useParams();
  const navigate = useNavigate();
  const { data, isLoading } = useSessionsList({ q: query || undefined, ...filters });

  if (isLoading) return <div className="flex items-center justify-center p-8 text-muted-foreground"><Loader2 className="size-4 animate-spin mr-2"/> Loading…</div>;
  if (!data?.items.length) return <div className="p-8 text-muted-foreground text-sm text-center">세션 없음</div>;

  return (
    <div className="divide-y divide-border">
      {data.items.map((s) => (
        <SessionListItem
          key={s.id}
          session={s}
          selected={s.id === id}
          onSelect={() => navigate(`/sessions/${s.id}`)}
        />
      ))}
    </div>
  );
}
```

### 6. SessionListItem 컴포넌트

`web/src/components/SessionListItem.tsx`:
- 첫 줄: 프로젝트 (없으면 agent), 우측에 날짜 (YYYY-MM-DD)
- 둘째 줄: summary 1-2줄 truncate
- 셋째 줄: 태그 칩들 (tagColor) + favorite 별표
- 선택 상태면 좌측 보더 강조

### 7. SessionDetailRoute

`web/src/routes/SessionDetailRoute.tsx`:
```tsx
import { useParams } from "react-router";
import { useSession } from "@/hooks/useSessions";
import { MarkdownView } from "@/components/MarkdownView";
import { Loader2 } from "lucide-react";

export default function SessionDetailRoute() {
  const { id } = useParams();
  const { data, isLoading, error } = useSession(id, true);  // full=true

  if (isLoading) return <div className="p-8 flex items-center text-muted-foreground"><Loader2 className="size-4 animate-spin mr-2"/> Loading…</div>;
  if (error) return <div className="p-8 text-rose-400">{error.message}</div>;
  if (!data) return null;

  // /api/get 응답 형식에 따라 분기 (markdown 본문이 어디 있는지)
  // 가정: { session: {...meta}, body: "<markdown>" } — 실제 구조는 do_get 검토 후 확정
  return (
    <div className="p-6 max-w-4xl">
      <SessionHeader session={data.session} />
      <MarkdownView content={data.body ?? ""} />
    </div>
  );
}
```

> `do_get` 응답 정확한 형태 확인 필요. `crates/secall-core/src/mcp/server.rs`의 `do_get()` 반환 구조 검사 후 타입 결정. (`tool-request:rawq:do_get` 또는 `Read`로 확인)

### 8. 훅들

`web/src/hooks/useSessions.ts`:
```ts
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

export function useSessionsList(params: Parameters<typeof api.listSessions>[0]) {
  return useQuery({
    queryKey: ["sessions", "list", params],
    queryFn: () => api.listSessions(params),
  });
}

export function useSession(id: string | undefined, full = false) {
  return useQuery({
    queryKey: ["sessions", "detail", id, full],
    queryFn: () => api.getSession(id!, full),
    enabled: !!id,
  });
}
```

검색 (recall) 훅도 추가 — 키워드 입력 시 `/api/recall` 사용할지, `/api/sessions?q=`로 통일할지 결정. 본 task는 `/api/sessions?q=`로 통일 (서버 측 `q`는 summary LIKE — 가벼운 검색). 시맨틱 검색이 필요하면 SearchBar에서 mode 전환 시 별도 훅으로 `/api/recall` 호출.

### 9. SessionEmptyState

```tsx
// web/src/routes/SessionsRoute.tsx 또는 별도 파일
function SessionEmptyState() {
  return (
    <div className="h-full flex items-center justify-center text-muted-foreground text-sm">
      좌측에서 세션을 선택하세요
    </div>
  );
}
```

### 10. MarkdownView

`web/src/components/MarkdownView.tsx`:
```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function MarkdownView({ content }: { content: string }) {
  return (
    <div className="prose prose-invert prose-sm max-w-none">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
    </div>
  );
}
```

`@tailwindcss/typography` 추가 (없으면 prose 클래스 미동작):
```bash
cd web && pnpm add -D @tailwindcss/typography
```
`tailwind.config.ts`에 plugin 등록.

## Dependencies

- Task 03 완료 (`/api/sessions`, `/api/projects`, `/api/agents` 가용)
- Task 05 완료 (Layout, api.ts, queryClient, store 등 셋업됨)
- 신규 npm 패키지: `@tailwindcss/typography`, (선택) shadcn `select`

## Verification

```bash
# 1. 의존성 설치
cd web && pnpm add -D @tailwindcss/typography
cd web && pnpm dlx shadcn@latest add select  # SessionFilters용

# 2. 타입 체크
cd web && pnpm typecheck

# 3. 빌드 성공
cd web && pnpm build

# 4. # Manual: 통합 검증
#   - 별도 터미널에 `cargo run -- serve --port 8080` 띄움 (실제 DB 사용)
#   - `cd web && pnpm dev`
#   - http://127.0.0.1:5173/sessions 접속
#     - 좌측에 세션 리스트 보임 (실제 DB의 세션들)
#     - 검색바에 키워드 입력 시 결과 필터링
#     - 프로젝트/에이전트 필터 select 동작
#     - 즐겨찾기 only 체크 시 빈 결과 (Task 03/04 후 즐겨찾기 데이터 없음 — 정상)
#     - 세션 클릭 시 우측에 마크다운 렌더
#     - URL이 /sessions/:id로 변경됨
#   - 좌측 사이드바에서 Daily/Wiki 메뉴 클릭 시 placeholder 표시 (Task 07에서 본격 구현)

# 5. Rust 측 영향 없음
cargo check --all-targets --all-features
```

## Risks

- **`/api/get` 응답 형태 미확인**: 본 task 작성 전 `do_get()` 구조 확인 필수. 만약 markdown body 분리 안 되어 있으면 클라이언트에서 파일 fetch (vault_path) 필요 — 그 경우 별도 엔드포인트 추가 검토. 일단 `do_get`이 markdown 또는 turns array를 반환한다고 가정
- **검색 성능**: `/api/sessions?q=` 는 summary LIKE 검색 — 큰 DB에서 느릴 수 있음. 인덱스 없음. 시맨틱 검색이 필요하면 `/api/recall` 모드 사용 유도
- **디바운스 300ms**: 너무 짧으면 매 키 입력마다 요청. 너무 길면 답답. 300ms는 표준값
- **react-markdown 코드블록 하이라이트**: MVP는 plain pre/code. 하이라이트 필요 시 `react-syntax-highlighter` 추가 — Phase 1
- **빈 검색어 처리**: 빈 q는 전체 리스트 반환. URLSearchParams에서 빈 값 제외 처리 필요
- **세션 ID URL escape**: encodeURIComponent 필요 (`api.ts`에 이미 적용)

## Scope boundary

수정 금지:
- `crates/` 전체 — Rust 백엔드는 Task 02, 03, 04에서 완료
- `web/src/routes/DailyRoute.tsx`, `WikiRoute.tsx` 본격 구현 — Task 07
- 그래프 오버레이 — Task 08
- 태그 편집 / 즐겨찾기 토글 — Task 07 (SessionDetail에서 헤더 정도 placeholder만)
- `.github/workflows/`, `README.md` — Task 09
