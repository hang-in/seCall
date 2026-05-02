import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { useUi } from "./store";

/**
 * 그래프 오버레이의 시작 노드 결정 로직.
 *
 * 우선순위:
 * 1) URL `/sessions/:id`의 :id (현재 보고 있는 세션)
 * 2) Zustand store의 `selectedSessionId`
 * 3) 최근 세션 1개 (api.listSessions의 첫 번째 항목)
 *
 * fallback fetch는 1, 2가 모두 없을 때만 활성화한다.
 */
export function useStartNode(): string | null {
  const params = useParams();
  const selectedFromStore = useUi((s) => s.selectedSessionId);
  const explicitFromUrl = params.id ?? null;

  const fallback = useQuery({
    queryKey: ["startNode", "latest"],
    queryFn: () => api.listSessions({ page: 1, page_size: 1 }),
    enabled: !explicitFromUrl && !selectedFromStore,
    staleTime: 60_000,
  });

  return (
    explicitFromUrl ??
    selectedFromStore ??
    (fallback.data?.items[0]?.id ?? null)
  );
}
