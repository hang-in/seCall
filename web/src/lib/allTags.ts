import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

/**
 * TagEditor 자동완성용 태그 사전.
 *
 * 단순 구현: 첫 페이지 100개 세션의 태그 합집합.
 * Phase 1에서 정확도가 필요하면 `/api/tags` 전용 엔드포인트로 교체한다.
 *
 * `useSetTags` mutation의 onSuccess가 ["allTags"] 캐시를 invalidate하므로
 * 신규 태그 추가 직후에도 자동완성 결과가 갱신된다.
 */
export function useAllTags(): string[] {
  const { data } = useQuery({
    queryKey: ["allTags"],
    queryFn: () => api.listSessions({ page: 1, page_size: 100 }),
    staleTime: 30_000,
  });
  if (!data) return [];
  const set = new Set<string>();
  for (const item of data.items) {
    for (const tag of item.tags) set.add(tag);
  }
  return Array.from(set).sort();
}
