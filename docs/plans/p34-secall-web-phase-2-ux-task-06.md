---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 06
parallel_group: A
depends_on: []
---

# Task 06 — 그래프 시각화 강화 (dagre 레이아웃 + 노드 색상/아이콘)

## Changed files

수정:
- `web/src/components/GraphCanvas.tsx` — dagre로 노드 자동 레이아웃, 노드 타입별 색상/아이콘, 엣지 라벨 토글
- `web/src/hooks/useGraph.ts` — 시그니처 변경 — random position 대신 type-aware data 반환
- `web/src/components/GraphOverlay.tsx` — 엣지 라벨 토글 버튼, 범례
- `web/package.json` — `@dagrejs/dagre` 추가

신규:
- `web/src/lib/graphStyle.ts` — 노드 타입별 색상/아이콘/스타일 매핑 + dagre layout 헬퍼

## Change description

### 1. 의존성 추가

```bash
cd web && pnpm add @dagrejs/dagre
```

### 2. `graphStyle.ts`

```ts
import dagre from "@dagrejs/dagre";
import type { Edge, Node } from "@xyflow/react";
import { File, Folder, GitCommit, Hash, MessageSquare, Tag, User, Wrench } from "lucide-react";
import type { ComponentType } from "react";

/**
 * 노드 타입별 시각 스타일.
 * graph_nodes.type: session | project | agent | issue | file | tech | topic | tool
 */
export const NODE_STYLE: Record<string, { color: string; icon: ComponentType<{ className?: string }>; label: string }> = {
  session:  { color: "#a78bfa", icon: MessageSquare, label: "세션" },
  project:  { color: "#22d3ee", icon: Folder,        label: "프로젝트" },
  agent:    { color: "#f59e0b", icon: User,          label: "에이전트" },
  issue:    { color: "#ef4444", icon: GitCommit,     label: "이슈" },
  file:     { color: "#10b981", icon: File,          label: "파일" },
  tech:     { color: "#3b82f6", icon: Hash,          label: "기술" },
  topic:    { color: "#ec4899", icon: Tag,           label: "주제" },
  tool:     { color: "#94a3b8", icon: Wrench,        label: "툴" },
};

export function nodeStyleFor(type: string | undefined) {
  return NODE_STYLE[type ?? "topic"] ?? NODE_STYLE.topic;
}

/**
 * dagre로 노드 위치 자동 배치. 입력 노드의 position만 갱신.
 */
export function layoutWithDagre(
  nodes: Node[],
  edges: Edge[],
  direction: "TB" | "LR" = "LR",
): Node[] {
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
    return {
      ...n,
      position: { x: pos.x - NODE_W / 2, y: pos.y - NODE_H / 2 },
    };
  });
}
```

### 3. `useGraph` 갱신

기존 random position 제거. 노드 type을 data에 보존:
```ts
setNodes((prev) => {
  const existing = new Set(prev.map(n => n.id));
  const newOnes = (res.results ?? []).filter(...).map(r => ({
    id: r.node_id,
    type: "default",
    position: { x: 0, y: 0 },  // 임시. layoutWithDagre가 갱신
    data: { label: r.label ?? r.node_id.slice(0, 8), nodeType: r.node_type ?? "topic", relation: r.relation },
  }));
  return [...prev, ...newOnes];
});
```

GraphCanvas에서 `useMemo`로 layoutWithDagre 적용:
```ts
const laidOut = useMemo(() => layoutWithDagre(nodes, edges), [nodes, edges]);
```

### 4. GraphCanvas — custom node + 엣지 라벨

xyflow의 nodeTypes로 custom node 등록:
```tsx
import { ReactFlow, Background, Controls, MiniMap, Handle, Position } from "@xyflow/react";
import { nodeStyleFor } from "@/lib/graphStyle";

function CustomNode({ data }: { data: { label: string; nodeType: string } }) {
  const style = nodeStyleFor(data.nodeType);
  const Icon = style.icon;
  return (
    <div
      className="px-3 py-1.5 rounded-md border text-xs font-medium flex items-center gap-1.5"
      style={{ borderColor: style.color, color: style.color, background: "rgba(0,0,0,0.6)" }}
    >
      <Handle type="target" position={Position.Left} style={{ background: style.color }} />
      <Icon className="size-3" />
      <span className="truncate max-w-[120px]">{data.label}</span>
      <Handle type="source" position={Position.Right} style={{ background: style.color }} />
    </div>
  );
}

const NODE_TYPES = { default: CustomNode };
```

엣지 라벨 토글 state:
```tsx
const [showLabels, setShowLabels] = useState(true);
const styledEdges = useMemo(
  () => edges.map(e => ({ ...e, label: showLabels ? e.label : undefined })),
  [edges, showLabels],
);
```

### 5. MiniMap nodeColor 매핑

```tsx
<MiniMap
  pannable
  zoomable
  nodeColor={(n) => nodeStyleFor((n.data as any)?.nodeType).color}
/>
```

### 6. GraphOverlay 우상단 toolbar에 토글 버튼 추가

```tsx
<button onClick={() => setShowLabels(s => !s)}>
  {showLabels ? "라벨 숨김" : "라벨 표시"}
</button>
```

### 7. 범례

GraphOverlay 좌하단에 작은 legend (8개 노드 타입 색상 + 라벨). collapse 가능.

## Dependencies

- 외부 npm: `@dagrejs/dagre`
- 내부 task: 없음 (P32에서 GraphCanvas/useGraph 이미 존재)

## Verification

```bash
cd web && pnpm add @dagrejs/dagre
cd web && pnpm typecheck && pnpm build

# 수동:
# 그래프 오버레이 토글 → 노드가 dagre 레이아웃으로 정렬됨 (random 아님)
# 노드 타입별 색상 다름 (session=violet, project=cyan, ...)
# 노드 클릭 → 인접 expand → 새 노드 추가도 자동 재레이아웃
# 엣지 라벨 토글 동작
# MiniMap 색상이 노드 색상과 일치
# 범례 표시
```

## Risks

- **dagre 레이아웃 비용**: 노드 100개 이하면 무시 가능. 1000개 이상 시 layout 호출이 메인 스레드 차지 → 본 task 범위 외
- **노드 expand 시 재레이아웃 깜빡임**: 새 노드 추가 시 dagre 다시 계산 → 위치 점프. xyflow `fitView` 호출로 완화
- **xyflow attribution Pro 경고**: P32 결정대로 `proOptions: { hideAttribution: true }` 유지
- **노드 타입 unknown**: graph_nodes.type 외 새 타입 등장 시 fallback to topic. 상수 NODE_STYLE에 누락된 타입은 회색
- **dagre TS 타입**: `@dagrejs/dagre`는 자체 d.ts 제공. 별도 `@types/...` 불필요

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`, `.github/`, `README*`
- `web/src/components/{SearchBar,SessionFilters,Session*,TagEditor,Favorite*,Date*,Markdown*,Job*,Command*,HotkeyHelp*}.tsx`
- `web/src/routes/`
- `web/src/hooks/{useSessions,useDaily,useWiki,useTagMutations,useJob*,useRelated,useGlobalHotkeys,useListHotkeys}.ts`
- `web/src/lib/{api,types,store,allTags,tagColor,utils,queryClient,graphStartNode,highlight,hotkeyStore}.ts`
