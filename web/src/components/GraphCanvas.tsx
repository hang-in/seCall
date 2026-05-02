import { useMemo, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  Handle,
  Position,
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useGraphExpand, type GraphNodeData } from "@/hooks/useGraph";
import { useStartNode } from "@/lib/graphStartNode";
import { layoutWithDagre, nodeStyleFor } from "@/lib/graphStyle";
import { EyeOff, Eye } from "lucide-react";

interface Props {
  onNodeClick: (nodeId: string, nodeType: string) => void;
}

/**
 * 노드 타입별 색상/아이콘이 적용된 custom node.
 *
 * - data.nodeType: graph_nodes.type (session/project/agent/...)
 * - 좌/우 핸들로 dagre LR 레이아웃과 결을 맞춤
 */
function CustomNode({ data }: { data: GraphNodeData }) {
  const style = nodeStyleFor(data.nodeType);
  const Icon = style.icon;
  return (
    <div
      className="px-3 py-1.5 rounded-md border text-xs font-medium flex items-center gap-1.5"
      style={{
        borderColor: style.color,
        color: style.color,
        background: "rgba(0,0,0,0.6)",
      }}
    >
      <Handle
        type="target"
        position={Position.Left}
        style={{ background: style.color }}
      />
      <Icon className="size-3" />
      <span className="truncate max-w-[120px]">{data.label}</span>
      <Handle
        type="source"
        position={Position.Right}
        style={{ background: style.color }}
      />
    </div>
  );
}

const NODE_TYPES: NodeTypes = { default: CustomNode };

/**
 * @xyflow/react 캔버스 래퍼.
 *
 * - 시작 노드는 useStartNode로 자동 결정 (URL/store/fallback)
 * - 노드 클릭 시 onNodeClick + 해당 노드 expand (인접 노드 fetch)
 * - dagre LR 레이아웃으로 노드 자동 배치 (random position 폐기)
 * - 노드 타입별 색상/아이콘 — CustomNode + MiniMap nodeColor 매핑
 * - 엣지 라벨 표시 토글 (기본 표시)
 * - colorMode="dark"로 다크 테마 적용
 * - proOptions.hideAttribution은 라이선스상 회색 영역이지만 task 지시서를 따름
 */
export function GraphCanvas({ onNodeClick }: Props) {
  const startNodeId = useStartNode();
  const { nodes, edges, expand } = useGraphExpand(startNodeId);
  const [showLabels, setShowLabels] = useState(true);

  // dagre 레이아웃 — nodes/edges가 바뀔 때만 재계산
  const laidOutNodes = useMemo(
    () => layoutWithDagre(nodes, edges, "LR"),
    [nodes, edges],
  );

  // 라벨 토글 — 원본 edge.label은 유지하고 표시만 가린다
  const styledEdges = useMemo(
    () => edges.map((e) => ({ ...e, label: showLabels ? e.label : undefined })),
    [edges, showLabels],
  );

  if (!startNodeId) {
    return (
      <div className="h-full flex items-center justify-center text-muted-foreground">
        시작 노드 없음
      </div>
    );
  }

  return (
    <div className="relative h-full w-full">
      <ReactFlow
        nodes={laidOutNodes}
        edges={styledEdges}
        nodeTypes={NODE_TYPES}
        onNodeClick={(_, node) => {
          const data = node.data as GraphNodeData | undefined;
          const type = data?.nodeType ?? "unknown";
          onNodeClick(node.id, type);
          expand(node.id);
        }}
        fitView
        colorMode="dark"
        proOptions={{ hideAttribution: true }}
      >
        <Background />
        <Controls />
        <MiniMap
          pannable
          zoomable
          nodeColor={(n) => {
            const d = n.data as GraphNodeData | undefined;
            return nodeStyleFor(d?.nodeType).color;
          }}
        />
      </ReactFlow>

      {/* 우상단 toolbar — 라벨 토글 (Overlay의 X 버튼과 좌측 정렬을 피해 right-16) */}
      <div className="absolute top-4 right-16 z-10">
        <button
          onClick={() => setShowLabels((s) => !s)}
          className="rounded-md p-2 hover:bg-accent border border-border bg-background flex items-center gap-1.5 text-xs"
          aria-label={showLabels ? "Hide edge labels" : "Show edge labels"}
        >
          {showLabels ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
          <span>{showLabels ? "라벨 숨김" : "라벨 표시"}</span>
        </button>
      </div>
    </div>
  );
}
