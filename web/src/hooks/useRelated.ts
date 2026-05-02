import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { useSession } from "@/hooks/useSessions";

/**
 * 관련 세션 추천 훅 (P34 Task 05).
 *
 * 세 가지 source를 결합한다:
 *  1) 그래프 인접 (`/api/graph?node_id=<id>&depth=1`) — `node_type === "session"` 필터
 *  2) 같은 프로젝트 (`/api/sessions?project=<project>`) — 자기 자신 제외
 *  3) 같은 태그 (`/api/sessions?tag=<tag>`) — 첫 번째 태그만 사용
 *
 * 각 source에서 최대 5개 → session_id로 dedup → 최대 10개 반환.
 * 빈 결과면 호출 측이 패널을 숨길 수 있도록 빈 배열을 그대로 노출.
 *
 * 그래프 응답 형태는 `crates/secall-core/src/mcp/server.rs`의 `do_graph_query` 참조.
 * `api.graph(...)`는 unknown을 반환하므로 호출 측에서 캐스팅한다.
 */
export interface RelatedItem {
  id: string;
  reason: string;
  title?: string;
  date?: string;
}

interface GraphResult {
  node_id: string;
  relation: string;
  direction?: string;
  node_type?: string;
  label?: string;
}

interface GraphResponse {
  query_node: string;
  depth: number;
  results: GraphResult[];
  count: number;
}

export function useRelated(sessionId: string | undefined): {
  items: RelatedItem[];
  isLoading: boolean;
} {
  const detail = useSession(sessionId, false);

  const graphQ = useQuery({
    queryKey: ["related", "graph", sessionId],
    queryFn: () => api.graph({ node_id: sessionId!, depth: 1 }),
    enabled: !!sessionId,
  });

  const project = detail.data?.project ?? undefined;
  const tag = detail.data?.tags?.[0];

  const projectQ = useQuery({
    queryKey: ["related", "project", project],
    queryFn: () => api.listSessions({ project, page_size: 6 }),
    enabled: !!project,
  });

  const tagQ = useQuery({
    queryKey: ["related", "tag", tag],
    queryFn: () => api.listSessions({ tag, page_size: 6 }),
    enabled: !!tag,
  });

  const items = useMemo<RelatedItem[]>(() => {
    if (!sessionId) return [];
    const seen = new Set<string>([sessionId]);
    const out: RelatedItem[] = [];

    // 1) 그래프 인접 세션
    const graphResults = (graphQ.data as GraphResponse | undefined)?.results;
    graphResults
      ?.filter((r) => r.node_type === "session" && r.node_id !== sessionId)
      .slice(0, 5)
      .forEach((r) => {
        if (!seen.has(r.node_id)) {
          seen.add(r.node_id);
          out.push({ id: r.node_id, reason: r.relation || "graph" });
        }
      });

    // 2) 같은 프로젝트
    projectQ.data?.items
      .filter((s) => s.id !== sessionId)
      .slice(0, 5)
      .forEach((s) => {
        if (!seen.has(s.id)) {
          seen.add(s.id);
          out.push({
            id: s.id,
            reason: `project:${s.project ?? ""}`,
            title: s.summary ?? undefined,
            date: s.date,
          });
        }
      });

    // 3) 같은 태그
    tagQ.data?.items
      .filter((s) => s.id !== sessionId)
      .slice(0, 5)
      .forEach((s) => {
        if (!seen.has(s.id)) {
          seen.add(s.id);
          out.push({
            id: s.id,
            reason: `tag:${tag}`,
            title: s.summary ?? undefined,
            date: s.date,
          });
        }
      });

    return out.slice(0, 10);
  }, [sessionId, graphQ.data, projectQ.data, tagQ.data, tag]);

  return {
    items,
    isLoading: graphQ.isLoading || projectQ.isLoading || tagQ.isLoading,
  };
}
