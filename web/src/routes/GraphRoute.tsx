import { useNavigate } from "react-router";
import { Loader2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { ObsidianGraph } from "@/components/ObsidianGraph";
import { api } from "@/lib/api";
import { useStartNode } from "@/lib/graphStartNode";

/**
 * `/graph` 라우트 — Obsidian-style force-directed graph (Stage 5).
 *
 * production 의 `/api/graph` 는 starting node + depth BFS. 전체 그래프 snapshot endpoint 가
 * 없어서 시작 노드(가장 최근 세션 또는 store 의 selectedSessionId)에서 depth=2 로 가져옴.
 *
 * 추후 backend `/api/graph/snapshot` (type 별 limit 합집합) 추가 시 단일 fetch 로 대체 가능.
 */

interface GraphApiResult {
  query_node: string;
  depth: number;
  results: Array<{
    node_id: string;
    node_type: string;
    label?: string;
    relation: string;
    direction: "in" | "out";
  }>;
  count: number;
}

export default function GraphRoute() {
  const startNodeId = useStartNode();
  const navigate = useNavigate();

  const { data, isLoading, error } = useQuery<GraphApiResult>({
    queryKey: ["graph", "expand", startNodeId, 2],
    queryFn: () =>
      api.graph({ node_id: startNodeId!, depth: 2 }) as Promise<GraphApiResult>,
    enabled: !!startNodeId,
    staleTime: 60_000,
  });

  if (!startNodeId) {
    return (
      <div className="h-full flex items-center justify-center text-t-small text-text-3 px-ds-6 text-center">
        시작 노드가 없습니다. /sessions 에서 세션을 먼저 선택하세요.
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center text-t-small text-text-3">
        <Loader2 className="size-4 animate-spin mr-ds-2" /> 그래프 로드 중…
      </div>
    );
  }

  if (error) {
    const msg = error instanceof Error ? error.message : String(error);
    return (
      <div className="h-full flex items-center justify-center px-ds-6">
        <div className="text-t-small text-status-danger whitespace-pre-wrap">
          그래프 로드 실패: {msg}
        </div>
      </div>
    );
  }

  if (!data) return null;

  // API 결과 → Obsidian 그래프 데이터로 변환.
  // 시작 노드 자기 자신을 1번째 노드로 추가하고, 결과 각 항목은 인접 노드 + 엣지.
  const startType = inferType(startNodeId);
  const nodes = [
    { id: startNodeId, type: startType, label: shortenLabel(startNodeId, startType) },
    ...data.results.map((r) => ({
      id: r.node_id,
      type: r.node_type,
      label: r.label ?? shortenLabel(r.node_id, r.node_type),
    })),
  ];
  // 중복 제거 (depth=2 면 path 따라 같은 노드 두 번 들어올 수 있음)
  const seenIds = new Set<string>();
  const uniqueNodes = nodes.filter((n) => {
    if (seenIds.has(n.id)) return false;
    seenIds.add(n.id);
    return true;
  });

  const edges = data.results.map((r) =>
    r.direction === "out"
      ? { source: startNodeId, target: r.node_id }
      : { source: r.node_id, target: startNodeId },
  );

  return (
    <div className="h-full w-full bg-[var(--bg)] flex">
      <div className="flex-1 relative min-w-0">
        <ObsidianGraph
          nodes={uniqueNodes}
          edges={edges}
          onSessionClick={(sid) => navigate(`/sessions/${encodeURIComponent(sid)}`)}
        />
      </div>
      <aside className="w-[260px] shrink-0 border-l border-hairline bg-[var(--surface)] p-ds-4 overflow-auto">
        <div className="space-y-ds-4">
          <section>
            <div className="eyebrow mb-ds-2">Filters</div>
            <div className="text-t-meta text-text-3 italic">
              (다음 단계에서 type 별 toggle 추가)
            </div>
          </section>
          <section>
            <div className="eyebrow mb-ds-2">Stats</div>
            <div className="space-y-ds-1 text-t-small text-text-2">
              <div className="flex items-center justify-between">
                <span>Nodes</span>
                <span className="font-mono text-text-3 tabular-nums">
                  {uniqueNodes.length}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>Edges</span>
                <span className="font-mono text-text-3 tabular-nums">{edges.length}</span>
              </div>
              <div className="flex items-center justify-between">
                <span>Depth</span>
                <span className="font-mono text-text-3 tabular-nums">{data.depth}</span>
              </div>
            </div>
          </section>
          <section>
            <div className="eyebrow mb-ds-2">Start</div>
            <div className="font-mono text-t-mono text-text-3 break-all">
              {startNodeId}
            </div>
          </section>
        </div>
      </aside>
    </div>
  );
}

function inferType(nodeId: string): string {
  const i = nodeId.indexOf(":");
  return i > 0 ? nodeId.slice(0, i) : "session";
}

function shortenLabel(nodeId: string, type: string): string {
  if (type === "session") {
    const after = nodeId.indexOf(":") + 1;
    const uuid = nodeId.slice(after);
    return uuid.slice(0, 8);
  }
  const i = nodeId.indexOf(":");
  return i > 0 ? nodeId.slice(i + 1) : nodeId;
}
