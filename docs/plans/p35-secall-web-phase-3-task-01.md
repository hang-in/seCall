---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p35-secall-web-phase-3
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — web `useAllTags`를 `/api/tags`로 전환

## Changed files

수정:
- `web/src/lib/types.ts` — `TagCount` 타입 추가 (`/api/tags` 응답)
- `web/src/lib/api.ts` — `listTags(withCounts?)` 메서드 추가 (api 객체 안)
- `web/src/lib/allTags.ts:1-25` — sessions 100건 휴리스틱 제거하고 `api.listTags()` 호출로 전면 교체. 함수 시그니처(반환 `string[]`)는 유지하여 TagEditor/SessionFilters는 무수정.

신규: 없음

## Change description

### 1. `TagCount` 타입 (types.ts)

`SearchMode` 정의 근처에 추가:

```ts
/** `/api/tags?with_counts=true` 응답의 한 항목. 백엔드 `TagCount` 직렬화 형태. */
export interface TagCount {
  name: string;
  count: number;
}

/** `/api/tags` 응답. with_counts 분기에 따라 결과 형태가 다름. */
export interface TagsResponse {
  tags: TagCount[] | string[];
}
```

### 2. `api.listTags` (api.ts)

`api` 객체의 `listAgents` 다음에 추가:

```ts
listTags: (withCounts: boolean = true) =>
  jfetch<TagsResponse>(
    `/api/tags?with_counts=${withCounts ? "true" : "false"}`,
  ),
```

import에 `TagsResponse` 추가 (types.ts에서).

### 3. `useAllTags` 전면 교체 (allTags.ts)

기존 25줄을 다음으로 교체:

```ts
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { TagCount } from "@/lib/types";

/**
 * 백엔드 `/api/tags` (P35 Task 01)에서 정규화된 전체 태그를 가져온다.
 *
 * - 카운트 포함 — `useTagCounts()`에서 사용. UI 우선순위/뱃지에 활용 가능.
 * - 정렬: 백엔드가 count DESC, name ASC로 정렬하여 반환.
 * - `useSetTags` mutation의 onSuccess가 ["allTags"] 캐시를 invalidate.
 *   (P35: 키 호환을 위해 ["allTags"] 그대로 사용)
 */
function useAllTagsRaw() {
  return useQuery({
    queryKey: ["allTags"],
    queryFn: () => api.listTags(true),
    staleTime: 5 * 60_000, // 5분 — 태그가 자주 변하지 않음
  });
}

/** TagEditor/SessionFilters용 — 이름 배열만 반환 (정렬 보존). */
export function useAllTags(): string[] {
  const { data } = useAllTagsRaw();
  if (!data) return [];
  return (data.tags as TagCount[]).map((t) => t.name);
}

/** 사용 빈도까지 필요한 호출처용. */
export function useTagCounts(): TagCount[] {
  const { data } = useAllTagsRaw();
  if (!data) return [];
  return data.tags as TagCount[];
}
```

기존 `useAllTags(): string[]` 시그니처가 그대로 유지되므로 TagEditor / SessionFilters는 수정 불필요.

### 4. mutation invalidate 키 확인

`web/src/hooks/useTagMutations.ts`에서 `useSetTags`의 onSuccess가 `["allTags"]`를 invalidate하는지 확인. 본 task에서는 같은 키 그대로 사용하므로 자동으로 동작. invalidate 누락이 있으면 본 task에서 수정 가능.

## Dependencies

- 외부 npm: 없음
- 내부 task: **Task 00 완료 필수** — `/api/tags` endpoint가 있어야 함

## Verification

```bash
cd /Users/d9ng/privateProject/seCall/web && pnpm typecheck
cd /Users/d9ng/privateProject/seCall/web && pnpm build

# 라이브 (Task 00 + Task 01 둘 다 완료 후, 서버 실행 필요):
# secall serve --bind 127.0.0.1:8080 &
# 브라우저 → /sessions → SessionFilters의 태그 chip autocomplete가 100건 한계 너머 태그도 표시
# DevTools Network → /api/tags 호출 1회만 (5분 캐시)
```

## Risks

- **Task 00 미완료 시**: `/api/tags`가 404 → useQuery error → useAllTags가 빈 배열 반환. UI는 깨지지 않지만 자동완성 제안 안 보임.
- **응답 형태 분기**: `with_counts=true`가 기본이므로 `TagsResponse.tags`는 `TagCount[]`만 사용. `string[]`은 `as TagCount[]` 캐스팅 시 런타임 에러 가능 → 본 코드는 항상 `withCounts=true`로 호출하므로 OK. `as` 캐스팅보다 안전한 방식 원하면 두 hook을 별도 fetcher로 분리.
- **invalidate 키 호환**: 기존 `useTagMutations.ts`가 `["allTags"]` 키 invalidate한다고 가정. 다르면 본 task에서 수정 필요 — 코드 확인 후 fix.
- **staleTime 5분**: 다른 사용자가 태그 추가해도 5분간 자동완성에 안 보임. invalidate는 본인 mutation에서만 트리거됨. 타 사용자 동기화 필요하면 SSE 구독 (Phase 4+).

## Scope boundary

수정 금지:
- `crates/` 전체 — Task 00 영역
- `web/src/components/{TagEditor,SessionFilters}.tsx` — `useAllTags`의 호출만 사용, 시그니처 그대로 유지하므로 무수정
- `web/src/components/SessionList.tsx` — Task 02 영역
- `web/src/routes/router.tsx`, `web/vite.config.ts` — Task 03 영역
- `web/src/hooks/{useSessions,useDaily,useWiki,useJob*,useGraph,useGlobalHotkeys,useListHotkeys,useRelated,useDebounce}.ts` — 무관
- `.github/`, `README*` — Task 04 영역
