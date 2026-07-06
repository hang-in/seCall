import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
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

/**
 * Phase 3 — 달력 뷰 날짜별 세션 수 (`/api/sessions/calendar`).
 *
 * `from`/`to` 는 표시 중인 월의 로컬 날짜 경계. `enabled`(기본 true)로 달력이
 * 접혀 있을 때 호출을 막을 수 있다. query key 에 인자가 포함돼 월 이동 시 자동 refetch.
 */
export type CalendarFilters = Pick<
  SessionsListParams,
  "project" | "agent" | "tag" | "tags" | "favorite" | "include_automated" | "q"
>;

export function useSessionCalendar(
  from: string,
  to: string,
  tzOffset: number,
  filters: CalendarFilters = {},
  opts: { enabled?: boolean } = {},
) {
  return useQuery({
    // filters 를 key 에 포함 → 필터 변경 시 배지 카운트 자동 refetch (리스트와 동기화).
    queryKey: ["sessions", "calendar", from, to, tzOffset, filters],
    queryFn: () => api.sessionsCalendar({ from, to, tzOffset, ...filters }),
    enabled: opts.enabled ?? true,
    staleTime: 60_000,
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

/**
 * 세션 삭제 — `DELETE /api/sessions/:id`.
 *
 * 성공 시 `["sessions"]` 쿼리(list/infinite/detail)를 무효화해 리스트가 자동 갱신된다.
 */
export function useDeleteSession() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteSession(id),
    // 낙관적 삭제 — 백엔드 응답을 기다리지 않고 캐시에서 즉시 제거해 UI "멈춤" 체감을 없앤다.
    // (백엔드 delete_session_full 의 turns_fts 풀스캔 지연은 별건으로 근본 수정 예정.)
    onMutate: async (id: string) => {
      await qc.cancelQueries({ queryKey: ["sessions"] });
      await qc.cancelQueries({ queryKey: ["recall"] });
      // 낙관 업데이트 대상 쿼리만 정밀 백업 — ["sessions"] 전체는 detail 쿼리까지
      // 잡아 롤백 시 무관한 상세 변경이 유실될 수 있어 제외.
      const prev = [
        ...qc.getQueriesData({ queryKey: ["sessions", "infinite"] }),
        ...qc.getQueriesData({ queryKey: ["sessions", "list"] }),
        ...qc.getQueriesData({ queryKey: ["recall"] }),
      ];
      // 무한 스크롤 리스트: pages[].items 제거 + total 차감(카운트/끝 표시 정합).
      qc.setQueriesData(
        { queryKey: ["sessions", "infinite"] },
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (old: any) =>
          old?.pages
            ? {
                ...old,
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                pages: old.pages.map((p: any) => {
                  // eslint-disable-next-line @typescript-eslint/no-explicit-any
                  const items = p.items.filter((s: any) => s.id !== id);
                  const removed = p.items.length - items.length;
                  return { ...p, items, total: Math.max(0, p.total - removed) };
                }),
              }
            : old,
      );
      // 단일 리스트: items 제거 + total 차감.
      qc.setQueriesData(
        { queryKey: ["sessions", "list"] },
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (old: any) => {
          if (!old?.items) return old;
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const items = old.items.filter((s: any) => s.id !== id);
          const removed = old.items.length - items.length;
          return { ...old, items, total: Math.max(0, (old.total ?? 0) - removed) };
        },
      );
      // 시맨틱 결과: results 제거 + count 차감.
      qc.setQueriesData(
        { queryKey: ["recall"] },
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (old: any) => {
          if (!old?.results) return old;
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const results = old.results.filter((r: any) => r.session_id !== id);
          const removed = old.results.length - results.length;
          return { ...old, results, count: Math.max(0, (old.count ?? 0) - removed) };
        },
      );
      return { prev };
    },
    onError: (_err, _id, ctx) => {
      // 실패 시 스냅샷 롤백
      ctx?.prev?.forEach(([key, data]) => qc.setQueryData(key, data));
    },
    onSettled: () => {
      // 서버 상태와 최종 동기화
      qc.invalidateQueries({ queryKey: ["sessions"] });
      qc.invalidateQueries({ queryKey: ["recall"] });
    },
  });
}
