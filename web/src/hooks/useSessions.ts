import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { SessionFilterState, SessionsListParams } from "@/lib/types";

/**
 * 세션 리스트 조회 — `/api/sessions?q=...&project=...&...`
 *
 * 서버에서 `start_time DESC`로 정렬되며, `q`는 summary LIKE 검색이다.
 * 빈 문자열 q는 호출 측에서 undefined로 정리해서 넘긴다 (URLSearchParams 노이즈 방지).
 *
 * P34 Task 03: `params.tags`(string[])가 query key에 포함되어 있어 다중 태그
 * 변경 시 자동 refetch된다. (`["sessions","list",params]` — params 전체가 dep)
 */
export function useSessionsList(params: SessionsListParams) {
  return useQuery({
    queryKey: ["sessions", "list", params],
    queryFn: () => api.listSessions(params),
    placeholderData: (prev) => prev,
  });
}

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

/** 세션 상세 (`/api/get`). full=true면 마크다운 본문(`content`) 포함. */
export function useSession(id: string | undefined, full = false) {
  return useQuery({
    queryKey: ["sessions", "detail", id, full],
    queryFn: () => api.getSession(id!, full),
    enabled: !!id,
  });
}

/** 프로젝트 목록 (필터 select용). */
export function useProjects() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: () => api.listProjects(),
    staleTime: 60_000,
  });
}

/** 에이전트 목록 (필터 select용). */
export function useAgents() {
  return useQuery({
    queryKey: ["agents"],
    queryFn: () => api.listAgents(),
    staleTime: 60_000,
  });
}

/**
 * 시맨틱 검색 — `/api/recall` (mode=semantic).
 *
 * - query가 비어 있으면 호출하지 않는다 (`enabled` && trim 검사).
 * - Ollama 미설치 등 vector backend 비활성 상태에서는 백엔드가
 *   `{ results: [], count: 0 }`만 반환하고 에러를 던지지 않는다.
 *   → 호출 측이 `data.count === 0`로 graceful 안내를 띄울 것.
 * - filters의 project/agent만 서버로 전달 (date/tag/favorite는 do_recall이 지원 안 함).
 * - `placeholderData: prev`로 디바운스 입력 중 깜빡임 방지.
 */
export function useSemanticRecall(
  query: string,
  filters: SessionFilterState,
  opts: { enabled: boolean },
) {
  const trimmed = query.trim();
  return useQuery({
    queryKey: ["recall", "semantic", trimmed, filters.project, filters.agent],
    queryFn: () =>
      api.recall({
        query: trimmed,
        mode: "semantic",
        project: filters.project,
        agent: filters.agent,
        limit: 30,
      }),
    enabled: opts.enabled && trimmed.length > 0,
    staleTime: 60_000,
    placeholderData: (prev) => prev,
  });
}
