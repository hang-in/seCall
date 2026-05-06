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

  // Backend 의 graph_nodes/edges 는 type prefix 를 포함한 ID(`session:UUID`) 를 사용.
  // 시작 노드는 항상 session 이므로 raw UUID 에 `session:` 접두를 붙여 backend 와 일치시킨다.
  // (이후 expand 시엔 backend 응답의 r.node_id 가 이미 prefixed 라 별도 처리 불필요.)
  const raw =
    explicitFromUrl ?? selectedFromStore ?? fallback.data?.items[0]?.id ?? null;
  if (!raw) return null;
  // raw 가 이미 `session:` 접두를 포함한 경우(직접 호출처 등) 중복 접두 방지.
  return raw.startsWith("session:") ? raw : `session:${raw}`;
}
