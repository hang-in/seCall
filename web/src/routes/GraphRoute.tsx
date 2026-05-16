import { useMemo, useState } from "react";
import { useNavigate } from "react-router";
import { Loader2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { ObsidianGraph } from "@/components/ObsidianGraph";
import { api } from "@/lib/api";

/**
 * `/graph` 라우트 — Obsidian-style force-directed graph (Stage 9).
 *
 * `GET /api/graph/snapshot` 한 번 fetch — project / topic / agent / tool 노드 전부 +
 * session 노드는 degree 상위 N. 이전(stage 5) 의 single-start BFS 보다 훨씬 풍부.
 */

interface GraphNode {
  id: string;
  type: string;
  label: string;
}

interface GraphEdge {
  source: string;
  target: string;
  relation?: string;
}

interface GraphSnapshot {
  nodes: GraphNode[];
  edges: GraphEdge[];
  node_count: number;
  edge_count: number;
  /** P64: filter 후 in-set 인 총 엣지 수 (truncate 전). */
  total_edges_in_set?: number;
  session_limit: number;
  /** P64: server 가 적용한 edge cap. */
  edge_limit?: number;
  /** P64: edges 가 cap 에 의해 잘렸는지. */
  edges_truncated?: boolean;
}

export default function GraphRoute() {
  const navigate = useNavigate();
  const [sessionLimit] = useState(80);
  // P64: edge_limit default 500 (server clamp 50~5000). 큰 그래프에서 force-
  // simulation 멈춤 차단. 향후 사용자 조절 UI 가능.
  const [edgeLimit] = useState(500);
  const [hiddenTypes, setHiddenTypes] = useState<Set<string>>(new Set());

  const { data, isLoading, error } = useQuery<GraphSnapshot>({
    queryKey: ["graph", "snapshot", sessionLimit, edgeLimit],
    queryFn: () =>
      api.graphSnapshot(sessionLimit, edgeLimit) as Promise<GraphSnapshot>,
    staleTime: 60_000,
  });

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

  if (!data || data.nodes.length === 0) {
    return (
      <div className="h-full flex items-center justify-center px-ds-6 text-center">
        <div className="text-t-small text-text-3">
          그래프가 비어 있습니다. <code className="font-mono mx-1">secall sync</code> 또는{" "}
          <code className="font-mono mx-1">secall graph rebuild</code> 를 먼저 실행하세요.
        </div>
      </div>
    );
  }

  return (
    <div className="h-full w-full bg-[var(--bg)] flex">
      <div className="flex-1 relative min-w-0">
        <ObsidianGraph
          nodes={data.nodes}
          edges={data.edges}
          hiddenTypes={hiddenTypes}
          onSessionClick={(sid) => navigate(`/sessions/${encodeURIComponent(sid)}`)}
        />
      </div>
      <GraphSidebar
        nodes={data.nodes}
        edges={data.edges}
        hiddenTypes={hiddenTypes}
        onToggleType={(t) =>
          setHiddenTypes((prev) => {
            const next = new Set(prev);
            if (next.has(t)) next.delete(t);
            else next.add(t);
            return next;
          })
        }
        sessionLimit={data.session_limit}
        edgeLimit={data.edge_limit}
        totalEdgesInSet={data.total_edges_in_set}
        edgesTruncated={data.edges_truncated}
      />
    </div>
  );
}

interface SidebarProps {
  nodes: GraphNode[];
  edges: GraphEdge[];
  hiddenTypes: Set<string>;
  onToggleType: (t: string) => void;
  sessionLimit: number;
  edgeLimit?: number;
  totalEdgesInSet?: number;
  edgesTruncated?: boolean;
}

const TYPE_DOT: Record<string, string> = {
  project: "var(--accent)",
  topic: "var(--info)",
  agent: "var(--success)",
  tool: "var(--warn)",
  session: "var(--text-3)",
};

function GraphSidebar({
  nodes,
  edges,
  hiddenTypes,
  onToggleType,
  sessionLimit,
  edgeLimit,
  totalEdgesInSet,
  edgesTruncated,
}: SidebarProps) {
  // 타입별 카운트 (전체)
  const typeCounts = useMemo(() => {
    const m = new Map<string, number>();
    for (const n of nodes) m.set(n.type, (m.get(n.type) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  }, [nodes]);

  // visible 카운트
  const visibleNodeCount = nodes.filter((n) => !hiddenTypes.has(n.type)).length;
  const visibleEdgeCount = edges.filter((e) => {
    const src = nodes.find((n) => n.id === e.source);
    const tgt = nodes.find((n) => n.id === e.target);
    return src && tgt && !hiddenTypes.has(src.type) && !hiddenTypes.has(tgt.type);
  }).length;

  return (
    <aside className="w-[260px] shrink-0 border-l border-hairline bg-[var(--surface)] p-ds-4 overflow-auto">
      <div className="space-y-ds-4">
        <section>
          <div className="eyebrow mb-ds-2">Filters</div>
          <div className="space-y-ds-1">
            {typeCounts.map(([t, n]) => {
              const visible = !hiddenTypes.has(t);
              return (
                <label
                  key={t}
                  className="flex items-center gap-ds-2 px-ds-2 py-ds-1 rounded-md cursor-pointer hover:bg-surface-2 transition-colors duration-fast ease-ds"
                >
                  <input
                    type="checkbox"
                    checked={visible}
                    onChange={() => onToggleType(t)}
                    className="size-3.5 cursor-pointer accent-[var(--accent)]"
                  />
                  <span
                    className="size-1.5 rounded-full shrink-0"
                    style={{ background: TYPE_DOT[t] ?? "var(--text-3)" }}
                    aria-hidden
                  />
                  <span
                    className={`text-t-small flex-1 ${
                      visible ? "text-text-2" : "text-text-4"
                    }`}
                  >
                    {t}
                  </span>
                  <span className="font-mono text-t-meta text-text-3 tabular-nums">
                    {n}
                  </span>
                </label>
              );
            })}
          </div>
        </section>

        <section>
          <div className="eyebrow mb-ds-2">Stats</div>
          <div className="space-y-ds-1 text-t-small text-text-2">
            <div className="flex items-center justify-between">
              <span>Nodes</span>
              <span className="font-mono text-text-3 tabular-nums">
                {visibleNodeCount} / {nodes.length}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span>Edges</span>
              <span className="font-mono text-text-3 tabular-nums">
                {visibleEdgeCount} / {edges.length}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span>Session limit</span>
              <span className="font-mono text-text-3 tabular-nums">
                {sessionLimit}
              </span>
            </div>
            {edgeLimit !== undefined && (
              <div className="flex items-center justify-between">
                <span>Edge limit</span>
                <span className="font-mono text-text-3 tabular-nums">
                  {edgeLimit}
                </span>
              </div>
            )}
            {edgesTruncated && totalEdgesInSet !== undefined && (
              <div className="flex items-center justify-between text-status-warn">
                <span>Edges truncated</span>
                <span className="font-mono tabular-nums">
                  {totalEdgesInSet - edges.length} 숨김
                </span>
              </div>
            )}
          </div>
        </section>

        <section>
          <div className="eyebrow mb-ds-2">Note</div>
          <div className="text-t-meta text-text-3 leading-relaxed">
            project / topic / agent / tool 은 전부, session 은 degree 상위 {sessionLimit} 만 표시.
            엣지는 우선순위 (session↔session &gt; session↔topic/tool &gt; …) 로 정렬 후 상위 {edgeLimit ?? 500} 까지.
            그 외 인접 관계는 force-simulation 결과 위치에서 사라질 수 있습니다.
          </div>
        </section>
      </div>
    </aside>
  );
}
