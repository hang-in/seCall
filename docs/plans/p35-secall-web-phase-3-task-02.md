---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p35-secall-web-phase-3
task_id: 02
parallel_group: A
depends_on: []
---

# Task 02 — SessionList 무한 스크롤

## Changed files

수정:
- `web/src/hooks/useSessions.ts` — `useInfiniteSessions(params)` 신규 추가 (`useInfiniteQuery` 기반). 기존 `useSessionsList`는 `useAllTags` 등 다른 호출처가 사용 중이므로 유지.
- `web/src/components/SessionList.tsx:62-67, 174-188` — keyword 모드를 `useSessionsList` → `useInfiniteSessions`로 교체. 기존 페이지네이션 안내(`{data.items.length} / {data.total} (페이지네이션 Phase 1)`)를 sentinel + 자동 로딩 표시로 교체. semantic 모드는 그대로.

신규:
- `web/src/hooks/useInfiniteScroll.ts` — IntersectionObserver 래퍼 훅. sentinel ref가 viewport에 들어오면 callback 호출.

## Change description

### 1. `useInfiniteScroll` (신규 hook)

```ts
import { useEffect, useRef } from "react";

/**
 * sentinel 엘리먼트가 viewport 안에 들어오면 onIntersect 호출.
 *
 * - rootMargin "200px": 끝에서 200px 전에 미리 fetch (체감 끊김 감소)
 * - hasMore=false면 observer 미설정 → 마지막 페이지 도달 후 호출 안 됨
 * - enabled=false (예: isFetching 중)면 일시 중단
 */
export function useInfiniteScroll(opts: {
  onIntersect: () => void;
  hasMore: boolean;
  enabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const { onIntersect, hasMore, enabled = true } = opts;

  useEffect(() => {
    const el = ref.current;
    if (!el || !hasMore || !enabled) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            onIntersect();
            break;
          }
        }
      },
      { rootMargin: "200px 0px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [onIntersect, hasMore, enabled]);

  return ref;
}
```

### 2. `useInfiniteSessions` (useSessions.ts에 추가)

기존 `useSessionsList` 아래에 추가:

```ts
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";

/**
 * 무한 스크롤 — `/api/sessions?page=N&page_size=...`.
 *
 * - 백엔드는 `{ items, total, page, page_size }` 반환 (P32).
 * - getNextPageParam: `items.length < page_size` 또는 누적 >= total이면 더 없음.
 * - keyword 모드 전용. semantic 모드는 do_recall이 페이지네이션 없으므로 useSemanticRecall 그대로.
 */
export function useInfiniteSessions(
  params: Omit<SessionsListParams, "page" | "page_size">,
  pageSize: number = 50,
) {
  return useInfiniteQuery({
    queryKey: ["sessions", "infinite", params, pageSize],
    queryFn: ({ pageParam }) =>
      api.listSessions({ ...params, page: pageParam, page_size: pageSize }),
    initialPageParam: 1,
    getNextPageParam: (lastPage) => {
      const fetchedSoFar = lastPage.page * lastPage.page_size;
      if (lastPage.items.length < lastPage.page_size) return undefined;
      if (fetchedSoFar >= lastPage.total) return undefined;
      return lastPage.page + 1;
    },
    placeholderData: (prev) => prev,
  });
}
```

### 3. `SessionList` 교체 (keyword 모드)

`web/src/components/SessionList.tsx`:

```tsx
// imports에 추가:
import { useInfiniteScroll } from "@/hooks/useInfiniteScroll";
// useSessionsList 대신:
import { useInfiniteSessions, useSemanticRecall } from "@/hooks/useSessions";

// keywordList 부분 교체:
const keywordList = useInfiniteSessions(
  {
    q: trimmed === "" ? undefined : trimmed,
    ...filters,
  },
  pageSize,
);

// 모든 페이지의 items를 평탄화
const allItems: Session[] = (keywordList.data?.pages ?? []).flatMap((p) => p.items);
const total = keywordList.data?.pages[0]?.total ?? 0;

// hotkeyItems 계산 — semantic이면 변환, keyword면 allItems
const hotkeyItems: Session[] = useSemantic
  ? semanticList.data ? recallToSessions(semanticList.data.results) : []
  : allItems;

// 기존 useListHotkeys 호출 그대로 유지

// keyword render 부분:
const sentinelRef = useInfiniteScroll({
  onIntersect: () => keywordList.fetchNextPage(),
  hasMore: keywordList.hasNextPage ?? false,
  enabled: !keywordList.isFetchingNextPage,
});

if (keywordList.isLoading) { /* 기존 그대로 */ }
if (keywordList.isError)   { /* 기존 그대로 (error 객체 위치가 keywordList.error) */ }
if (allItems.length === 0) { /* 기존 그대로 */ }

return (
  <div>
    {keywordList.isFetching && !keywordList.isFetchingNextPage && (
      <div className="px-3 py-1 text-[10px] text-muted-foreground border-b border-border">
        업데이트 중…
      </div>
    )}
    <div className="divide-y divide-border">
      {allItems.map((s) => (
        <SessionListItem
          key={s.id}
          session={s}
          query={query}
          selected={s.id === id}
          onSelect={() => navigate(`/sessions/${encodeURIComponent(s.id)}`)}
        />
      ))}
    </div>

    {/* sentinel — IntersectionObserver 타겟 */}
    <div ref={sentinelRef} className="h-10" aria-hidden />

    {keywordList.isFetchingNextPage && (
      <div className="p-3 text-[11px] text-muted-foreground text-center border-t border-border flex items-center justify-center gap-2">
        <Loader2 className="size-3 animate-spin" /> 추가 로드 중…
      </div>
    )}

    {!keywordList.hasNextPage && allItems.length > 0 && allItems.length === total && (
      <div className="p-3 text-[10px] text-muted-foreground text-center border-t border-border">
        끝 — 총 {total} 세션
      </div>
    )}
  </div>
);
```

### 4. semantic 모드 무수정

semantic 모드는 `useSemanticRecall` 그대로 사용. 백엔드 `do_recall`이 페이지네이션 미지원이므로 단발 결과만 표시. 본 task의 무한 스크롤은 keyword 모드 한정.

## Dependencies

- 외부 npm: `@tanstack/react-query` (이미 사용 중) — `useInfiniteQuery`는 v5 정식 API
- 내부 task: 없음 (Task 00/01/03과 독립)

## Verification

```bash
cd /Users/d9ng/privateProject/seCall/web && pnpm typecheck
cd /Users/d9ng/privateProject/seCall/web && pnpm build

# 라이브 (서버 실행 + 100건 이상 세션 필요):
# secall serve --bind 127.0.0.1:8080 &
# 브라우저 /sessions → 스크롤 끝 도달 시 자동 로드, 마지막 페이지 도달 시 "끝 — 총 N 세션" 표시
# DevTools Network → /api/sessions?page=2 자동 호출 확인
# `]` 단축키로 page 2 이상 항목으로도 이동 가능 확인 (useListHotkeys가 allItems를 dep로 받으므로 자동)
```

## Risks

- **`useSessionsList` 호출처 영향**: 기존 `useSessionsList`는 `useAllTags`와 `useRelated`에서 사용 중. 본 task에서는 SessionList만 교체하므로 다른 호출처는 영향 없음. (`useAllTags`는 Task 01에서 별도로 `/api/tags`로 전환됨.)
- **filters 변경 시 캐시**: `queryKey`에 params 포함됨 → filters 변경 시 새 infinite query 생성 → 페이지 1부터 재조회. OK.
- **placeholderData prev**: 검색어 디바운스 입력 중에도 이전 페이지가 잠시 보임. UX 부드러움.
- **단축키 j/k 다음 페이지 끝 도달**: useListHotkeys가 hotkeyItems 끝 도달 시 더 이상 이동 안 함. 자동 fetchNextPage 트리거는 본 task 외 (Phase 4+).
- **rootMargin 200px**: 화면 최하단 200px 전에 미리 페치 시작 → 모바일에서 약간 일찍 로드. 적절.
- **IntersectionObserver 미지원 브라우저**: 모던 브라우저는 모두 지원. 폴백 불필요.

## Scope boundary

수정 금지:
- `crates/` 전체 — 백엔드와 무관
- `web/src/lib/{api,types,store,allTags,tagColor,utils,queryClient,graphStartNode,highlight,graphStyle}.ts` — 본 task와 무관 (api/types는 Task 01에서 변경)
- `web/src/components/*` — `SessionList.tsx` 외 모든 컴포넌트
- `web/src/routes/*` — Task 03 영역
- `web/vite.config.ts` — Task 03 영역
- `web/src/hooks/*` — `useSessions.ts` + 신규 `useInfiniteScroll.ts` 외
- `.github/`, `README*` — Task 04 영역
