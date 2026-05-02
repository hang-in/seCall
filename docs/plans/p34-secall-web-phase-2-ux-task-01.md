---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 01
parallel_group: A
depends_on: []
---

# Task 01 — 시맨틱 검색 모드 활성

## Changed files

수정:
- `web/src/components/SearchBar.tsx` — mode 토글 (keyword/semantic)을 callback으로 부모에 전달
- `web/src/components/SessionList.tsx` — `mode === "semantic"` 시 `/api/recall` 호출, 그 외엔 기존 `/api/sessions?q=`
- `web/src/hooks/useSessions.ts` — `useSemanticRecall(query, params)` 훅 신규
- `web/src/lib/api.ts` — 기존 `recall` 응답 타입을 `RecallResponse`로 강타입화
- `web/src/lib/types.ts` — `RecallResponse`, `RecallResultItem` 타입 추가
- `web/src/routes/SessionsRoute.tsx` — query/mode 양쪽 state 관리 + SessionList에 전달

신규: 없음

## Change description

### 1. mode 상태 끌어올리기

기존 SearchBar 내부 `mode` state를 부모(SessionsRoute)로 lift:
```tsx
const [query, setQuery] = useState("");
const [mode, setMode] = useState<SearchMode>("keyword");
const [filters, setFilters] = useState<SessionFilterState>({});
// SearchBar onChange={(q, m) => { setQuery(q); setMode(m); }}
// SessionList query={query} mode={mode} filters={filters}
```

### 2. SessionList 분기

```tsx
export function SessionList({ query, mode, filters }) {
  const useKeyword = mode === "keyword" || !query;
  const keywordList = useSessionsList({ q: query || undefined, ...filters }, { enabled: useKeyword });
  const semanticList = useSemanticRecall(query, filters, { enabled: !useKeyword });

  if (mode === "semantic") {
    if (semanticList.isError) {
      const msg = String(semanticList.error?.message || "");
      const ollamaDown = /vector|ollama|embedding/i.test(msg);
      return ollamaDown
        ? <div>...시맨틱 검색을 사용하려면 Ollama가 필요합니다...</div>
        : <div>{msg}</div>;
    }
    // semanticList.data를 SessionListItem 형태로 매핑하여 표시
  }
  // 기존 keywordList 흐름
}
```

### 3. `useSemanticRecall`

```ts
export function useSemanticRecall(
  query: string,
  filters: SessionFilterState,
  opts: { enabled: boolean },
) {
  return useQuery({
    queryKey: ["recall", "semantic", query, filters],
    queryFn: () => api.recall({ query, mode: "semantic", project: filters.project, agent: filters.agent, limit: 30 }),
    enabled: opts.enabled && query.trim().length > 0,
    staleTime: 60_000,
    placeholderData: (prev) => prev,
  });
}
```

### 4. `RecallResponse` 타입

`do_recall` 응답: `{ results: [{ session_id, turn_index, content, score, ... }], count, related_sessions }`. SessionListItem과 형태가 다름 — UI에서 매핑.

```ts
export interface RecallResultItem {
  session_id: string;
  turn_index?: number | null;
  content?: string | null;
  score?: number | null;
  agent?: string;
  project?: string | null;
  date?: string;
  // ... do_recall 응답의 SearchResult 구조
}

export interface RecallResponse {
  results: RecallResultItem[];
  count: number;
  related_sessions?: unknown[];
}
```

### 5. SessionList의 semantic 결과 표시

semantic 결과는 turn 단위. UI는 세션 리스트 패턴 유지하면서 score 표시 + 같은 session_id가 여러 번 나오면 dedup (서버 diversify_by_session으로 이미 처리되지만 클라이언트 안전망).

각 row 클릭 시 `navigate(/sessions/${session_id})`. 추가로 `?turn=${turn_index}` 쿼리 파라미터로 SessionDetail에서 해당 turn 스크롤 (옵션, P34 범위 안 — 단순화 위해 turn anchor는 P35).

### 6. Ollama 미설치 graceful

서버 응답이 `{ "results": [], "count": 0 }`이면 vector search 비활성 (Ollama 없음). UI에 "시맨틱 검색이 비활성 상태입니다 (Ollama 필요)" 안내.

`do_recall`은 vector 실패 시 `tracing::info!("vector search disabled (Ollama not available)")` 로그만 찍고 빈 결과 반환. error throw 안 함. 따라서 isError 분기보다 `count === 0 && query.trim()`인 경우 안내.

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음 (P32에서 SearchBar/useSessions 이미 존재)

## Verification

```bash
cd web && pnpm typecheck
cd web && pnpm build
cargo check --all-targets

# 수동: brew services start ollama && ollama pull bge-m3 후
# http://127.0.0.1:5173/sessions에서 mode를 semantic으로 토글하고 한국어 검색
```

## Risks

- **Ollama 미설치 시 UX**: 빈 결과 vs 명시적 안내 구분 필요. 백엔드가 명시적 에러 안 줌 → count=0 + ollama 안내 휴리스틱 필요
- **추가 `?turn=` 라우팅은 본 task 외**: SessionDetail에서 특정 turn으로 스크롤하는 기능은 P35
- **dedup**: do_recall이 `diversify_by_session(max=2)`로 이미 처리하지만 다중 query 조합 시 중복 가능
- **embedding 캐시 비용**: 동일 query 재호출 시 매번 embedding 새로. staleTime 60s로 클라이언트 캐시 의존

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`
- `web/src/routes/{SessionDetail,Daily,Wiki,Commands}Route.tsx`
- `web/src/components/{SessionFilters,SessionListItem,Session*}.tsx` (Header/TagEditor/Favorite 등 — 단 SessionList는 본 task)
- `web/src/components/Job*.tsx`, `Graph*.tsx`
- `.github/`, `README*`
