import { useState, useCallback, useEffect, useRef } from "react";
import type { Node, Edge } from "@xyflow/react";
import { api } from "@/lib/api";

/**
 * `/api/graph` 응답 (do_graph_query, crates/secall-core/src/mcp/server.rs:339)
 *
 * ```
 * {
 *   query_node: string,
 *   depth: number,
 *   results: Array<{
 *     node_id: string,
 *     relation: string,
 *     direction: "outgoing" | "incoming",
 *     node_type?: string,
 *     label?: string,
 *   }>,
 *   count: number,
 * }
 * ```
 */
interface GraphApiResponse {
  query_node: string;
  depth: number;
  results: Array<{
    node_id: string;
    relation: string;
    direction: string;
    node_type?: string;
    label?: string;
  }>;
  count: number;
}

/**
 * 그래프 노드 data 페이로드.
 *
 * - label: 화면에 표시되는 텍스트
 * - nodeType: graph_nodes.type — CustomNode/MiniMap 색상 결정에 사용
 *   (이전 키 'type'에서 변경 — xyflow Node.type과의 혼동 방지)
 */
export interface GraphNodeData extends Record<string, unknown> {
  label: string;
  nodeType: string;
}

interface GraphState {
  nodes: Node<GraphNodeData>[];
  edges: Edge[];
  expand: (nodeId: string) => void;
}

/**
 * 그래프 상태 관리 훅.
 *
 * - startNodeId가 정해지면 자동으로 1회 fetch (depth=1)
 * - expand(nodeId)를 호출하면 해당 노드의 인접 노드를 fetch해서 누적 머지
 * - 동일 노드 중복 fetch 방지 (expandedSet)
 * - direction에 따라 edge의 source/target 결정 (outgoing: query→neighbor)
 *
 * 노드 position은 임시 (0,0). GraphCanvas에서 dagre 레이아웃으로 재배치한다.
 */
export function useGraphExpand(startNodeId: string | null): GraphState {
  const [nodes, setNodes] = useState<Node<GraphNodeData>[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const expandedRef = useRef<Set<string>>(new Set());

  const fetchAndMerge = useCallback(async (nodeId: string) => {
    if (expandedRef.current.has(nodeId)) return;
    expandedRef.current.add(nodeId);

    const res = (await api.graph({ node_id: nodeId, depth: 1 })) as GraphApiResponse;
    if (!res || !Array.isArray(res.results)) return;

    setNodes((prev) => {
      const existing = new Map(prev.map((n) => [n.id, n] as const));

      // 1) 쿼리 노드 자신을 (없으면) 추가
      if (!existing.has(nodeId)) {
        existing.set(nodeId, {
          id: nodeId,
          type: "default",
          position: { x: 0, y: 0 },
          data: { label: nodeId, nodeType: "session" },
        });
      }

      // 2) 결과의 각 인접 노드 추가/머지
      for (const r of res.results) {
        if (existing.has(r.node_id)) {
          // label/nodeType만 업데이트 (없을 때 채워넣기)
          const cur = existing.get(r.node_id)!;
          existing.set(r.node_id, {
            ...cur,
            data: {
              label: r.label ?? cur.data.label,
              nodeType: r.node_type ?? cur.data.nodeType,
            },
          });
        } else {
          existing.set(r.node_id, {
            id: r.node_id,
            type: "default",
            position: { x: 0, y: 0 },
            data: {
              label: r.label ?? r.node_id,
              nodeType: r.node_type ?? "topic",
            },
          });
        }
      }
      return Array.from(existing.values());
    });

    setEdges((prev) => {
      const existingKeys = new Set(prev.map((e) => e.id));
      const additions: Edge[] = [];
      for (const r of res.results) {
        // direction: "outgoing" → query_node → r.node_id, "incoming" → r.node_id → query_node
        const isOutgoing = r.direction === "outgoing";
        const source = isOutgoing ? nodeId : r.node_id;
        const target = isOutgoing ? r.node_id : nodeId;
        const id = `${source}-[${r.relation}]->${target}`;
        if (existingKeys.has(id)) continue;
        existingKeys.add(id);
        additions.push({
          id,
          source,
          target,
          label: r.relation,
          animated: false,
        });
      }
      return [...prev, ...additions];
    });
  }, []);

  // 시작 노드가 바뀌면 상태 리셋 후 자동 fetch
  useEffect(() => {
    expandedRef.current = new Set();
    setNodes([]);
    setEdges([]);
    if (startNodeId) {
      void fetchAndMerge(startNodeId);
    }
  }, [startNodeId, fetchAndMerge]);

  return { nodes, edges, expand: fetchAndMerge };
}
