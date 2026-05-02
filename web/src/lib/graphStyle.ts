import dagre from "@dagrejs/dagre";
import type { Edge, Node } from "@xyflow/react";
import { File, Folder, GitCommit, Hash, MessageSquare, Tag, User, Wrench } from "lucide-react";
import type { ComponentType } from "react";

/**
 * 노드 타입별 시각 스타일.
 * graph_nodes.type: session | project | agent | issue | file | tech | topic | tool
 *
 * 누락된 타입은 nodeStyleFor()에서 topic으로 fallback (Risks 명시).
 */
export const NODE_STYLE: Record<
  string,
  { color: string; icon: ComponentType<{ className?: string }>; label: string }
> = {
  session: { color: "#a78bfa", icon: MessageSquare, label: "세션" },
  project: { color: "#22d3ee", icon: Folder, label: "프로젝트" },
  agent: { color: "#f59e0b", icon: User, label: "에이전트" },
  issue: { color: "#ef4444", icon: GitCommit, label: "이슈" },
  file: { color: "#10b981", icon: File, label: "파일" },
  tech: { color: "#3b82f6", icon: Hash, label: "기술" },
  topic: { color: "#ec4899", icon: Tag, label: "주제" },
  tool: { color: "#94a3b8", icon: Wrench, label: "툴" },
};

/** 알 수 없는 타입은 topic 스타일로 fallback. */
export function nodeStyleFor(type: string | undefined) {
  return NODE_STYLE[type ?? "topic"] ?? NODE_STYLE.topic;
}

/** 범례에 노출되는 8개 노드 타입 entries. 호출부에서 map. */
export const NODE_STYLE_ENTRIES: Array<
  [string, { color: string; icon: ComponentType<{ className?: string }>; label: string }]
> = Object.entries(NODE_STYLE);

/**
 * dagre로 노드 위치 자동 배치. 입력 노드의 position만 갱신.
 *
 * - direction: "LR" 권장. 좌→우 흐름이 그래프 탐색에서 가독성 우수.
 * - 노드 크기는 CustomNode 외형에 맞춰 추정 (160x50). 실측이 아님.
 * - 노드 0개일 때 dagre가 빈 그래프 처리하므로 그대로 빈 배열 반환.
 */
export function layoutWithDagre(
  nodes: Node[],
  edges: Edge[],
  direction: "TB" | "LR" = "LR",
): Node[] {
  if (nodes.length === 0) return nodes;

  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: direction, nodesep: 50, ranksep: 80 });

  const NODE_W = 160;
  const NODE_H = 50;
  for (const n of nodes) g.setNode(n.id, { width: NODE_W, height: NODE_H });
  for (const e of edges) g.setEdge(e.source, e.target);

  dagre.layout(g);

  return nodes.map((n) => {
    const pos = g.node(n.id);
    if (!pos) return n;
    return {
      ...n,
      position: { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
    };
  });
}
