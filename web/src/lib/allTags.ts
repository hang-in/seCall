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
