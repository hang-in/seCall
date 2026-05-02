---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 07
parallel_group: E
depends_on: [05]
---

# Task 07 — 그래프 폴딩 오버레이

## Changed files

수정:
- `web/src/routes/Layout.tsx` — `<GraphOverlay />` 마운트 추가
- `web/package.json` — 그래프 시각화 라이브러리 추가 (`reactflow` 또는 `cytoscape`)

신규:
- `web/src/components/GraphOverlay.tsx` — Sheet/Dialog 풀스크린 오버레이
- `web/src/components/GraphCanvas.tsx` — 노드/엣지 시각화 (선택한 라이브러리 래핑)
- `web/src/hooks/useGraph.ts` — `/api/graph` 호출 + 노드 확장(depth) 관리
- `web/src/lib/graphStartNode.ts` — 시작 노드 결정 로직 (현재 선택 세션 → 없으면 status로 최근 세션)

## Change description

### 1. 라이브러리 선택

후보:
- **reactflow (xyflow/react)** — 드래그/줌/미니맵 표준, ~150KB gzip
- **cytoscape.js** — 더 풍부한 레이아웃 알고리즘, 무겁다 (~300KB)
- **vis-network** — 클래식, 큰 그래프 잘 다룸

**권고: reactflow**. React 친화 + 충분한 기능 + 적당한 크기.

```bash
cd web && pnpm add @xyflow/react
```

### 2. GraphOverlay

`web/src/components/GraphOverlay.tsx`:
```tsx
import { useEffect } from "react";
import { X, Maximize2 } from "lucide-react";
import { useNavigate } from "react-router";
import { useUi } from "@/lib/store";
import { GraphCanvas } from "./GraphCanvas";

export function GraphOverlay() {
  const open = useUi((s) => s.graphOverlayOpen);
  const close = useUi((s) => s.toggleGraphOverlay);
  const setSelected = useUi((s) => s.setSelectedSession);
  const navigate = useNavigate();

  // ESC로 닫기
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, close]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 bg-background/95 backdrop-blur-sm">
      <div className="absolute top-4 right-4 z-10 flex gap-2">
        <button
          onClick={close}
          className="rounded-md p-2 hover:bg-accent border border-border"
          aria-label="Close graph"
        >
          <X className="size-5" />
        </button>
      </div>
      <div className="h-full w-full">
        <GraphCanvas
          onNodeClick={(nodeId, nodeType) => {
            if (nodeType === "session") {
              setSelected(nodeId);
              navigate(`/sessions/${nodeId}`);
              close();  // 자동 폴딩
            }
            // 다른 노드 타입(project, agent, topic, file 등)은 클릭 시 확장만
          }}
        />
      </div>
    </div>
  );
}
```

### 3. GraphCanvas

`web/src/components/GraphCanvas.tsx`:
```tsx
import { useEffect, useState } from "react";
import { ReactFlow, Background, Controls, MiniMap, Node, Edge } from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useUi } from "@/lib/store";
import { useGraphExpand } from "@/hooks/useGraph";
import { useStartNode } from "@/lib/graphStartNode";

interface Props {
  onNodeClick: (nodeId: string, nodeType: string) => void;
}

export function GraphCanvas({ onNodeClick }: Props) {
  const startNodeId = useStartNode();
  const { nodes, edges, expand } = useGraphExpand(startNodeId);

  if (!startNodeId) {
    return <div className="h-full flex items-center justify-center text-muted-foreground">시작 노드 없음</div>;
  }

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodeClick={(_, node) => {
        const type = (node.data as any)?.type ?? "unknown";
        onNodeClick(node.id, type);
        expand(node.id);  // 노드 확장 (인접 노드 fetch)
      }}
      fitView
      colorMode="dark"
      proOptions={{ hideAttribution: true }}
    >
      <Background />
      <Controls />
      <MiniMap />
    </ReactFlow>
  );
}
```

### 4. useGraph 훅

`web/src/hooks/useGraph.ts`:
```ts
import { useState, useCallback, useEffect } from "react";
import { Node, Edge } from "@xyflow/react";
import { api } from "@/lib/api";

interface GraphState {
  nodes: Node[];
  edges: Edge[];
  expand: (nodeId: string) => void;
}

export function useGraphExpand(startNodeId: string | null): GraphState {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [expandedSet, setExpandedSet] = useState<Set<string>>(new Set());

  const fetchAndMerge = useCallback(async (nodeId: string) => {
    if (expandedSet.has(nodeId)) return;
    setExpandedSet((s) => new Set([...s, nodeId]));

    const res: any = await api.graph({ node_id: nodeId, depth: 1 });
    // 응답 구조 가정: { nodes: [{id, type, label}], edges: [{source, target, relation}] }
    // do_graph_query 정확한 구조는 구현 시 확인

    setNodes((prev) => {
      const existing = new Set(prev.map((n) => n.id));
      const newOnes: Node[] = (res.nodes ?? []).filter((n: any) => !existing.has(n.id)).map((n: any, i: number) => ({
        id: n.id,
        type: "default",
        position: { x: Math.random() * 400, y: Math.random() * 400 },
        data: { label: n.label, type: n.type },
      }));
      return [...prev, ...newOnes];
    });
    setEdges((prev) => {
      const existing = new Set(prev.map((e) => `${e.source}-${e.target}-${(e as any).label}`));
      const newOnes: Edge[] = (res.edges ?? [])
        .map((e: any) => ({
          id: `${e.source}-${e.target}-${e.relation}`,
          source: e.source,
          target: e.target,
          label: e.relation,
          animated: false,
        }))
        .filter((e: Edge) => !existing.has(`${e.source}-${e.target}-${(e as any).label}`));
      return [...prev, ...newOnes];
    });
  }, [expandedSet]);

  // 시작 노드 자동 확장
  useEffect(() => {
    if (startNodeId) fetchAndMerge(startNodeId);
  }, [startNodeId, fetchAndMerge]);

  return { nodes, edges, expand: fetchAndMerge };
}
```

> 레이아웃은 random position으로 시작 — 추후 dagre/elk 알고리즘 도입 가능 (Phase 1+).

### 5. 시작 노드 결정

`web/src/lib/graphStartNode.ts`:
```ts
import { useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { useUi } from "./store";

export function useStartNode(): string | null {
  const params = useParams();
  const selectedFromStore = useUi((s) => s.selectedSessionId);
  const explicitFromUrl = params.id ?? null;

  // 1) URL에 세션 ID 있으면 사용
  // 2) store에 selected 있으면 사용
  // 3) 최근 세션 1개 fallback
  const fallback = useQuery({
    queryKey: ["startNode", "latest"],
    queryFn: () => api.listSessions({ page: 1, page_size: 1 }),
    enabled: !explicitFromUrl && !selectedFromStore,
  });

  return explicitFromUrl
    ?? selectedFromStore
    ?? (fallback.data?.items[0]?.id ?? null);
}
```

### 6. Layout에 마운트

`web/src/routes/Layout.tsx` 수정 — 이미 사이드바에 그래프 토글 버튼이 있음. 본 task에서는 `<GraphOverlay />` 마운트만 추가:
```tsx
import { GraphOverlay } from "@/components/GraphOverlay";

export default function Layout() {
  // ... 기존 코드
  return (
    <div className="flex h-screen bg-background text-foreground">
      <aside>...</aside>
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
      <GraphOverlay />  {/* ← 추가, fixed positioning이라 layout 영향 없음 */}
    </div>
  );
}
```

### 7. 사이드바 그래프 버튼 (Task 05에서 이미 추가됨)

`useUi.toggleGraphOverlay()` 호출하는 버튼이 사이드바 하단에 이미 존재 (Task 05 Layout 코드). 본 task는 마운트 + 동작만 추가.

## Dependencies

- Task 06 완료 (Layout, useUi store, 라우팅)
- 신규 npm 패키지: `@xyflow/react`

## Verification

```bash
# 1. 의존성 설치
cd web && pnpm add @xyflow/react

# 2. 타입 체크
cd web && pnpm typecheck

# 3. 빌드 성공
cd web && pnpm build

# 4. # Manual: 통합 검증
#   - `cargo run -- serve --port 8080` + `cd web && pnpm dev`
#   - http://127.0.0.1:5173/sessions/<실제 ID>
#     - 사이드바 하단 "Graph" 버튼 클릭 → 풀스크린 오버레이 열림
#     - 현재 세션이 중심 노드로 표시
#     - 인접 노드 (project, agent, file, topic, issue 등)가 엣지로 연결
#     - 노드 드래그/줌/팬 동작
#     - 다른 노드 클릭 → 해당 노드에서 확장 (인접 노드 fetch + 추가)
#     - 세션 노드 클릭 → 우측 pane에 해당 세션 로드 + 그래프 자동 닫힘
#     - ESC 또는 X 버튼으로 닫기
#   - http://127.0.0.1:5173/sessions (선택 안 한 상태) → 그래프 열면 최근 세션 1개에서 시작

# 5. Rust 측 영향 없음
cargo check --all-targets --all-features
```

## Risks

- **`/api/graph` 응답 구조 미확정**: `do_graph_query()` 정확한 형태 확인 필수. 본 task의 `useGraph` 훅에 `(res.nodes ?? [])`, `(res.edges ?? [])` 폴백 작성 — 구현 시 정확한 타입으로 교체. 만약 응답이 `{related: [{session_id, relation, ...}]}` 형태면 별도 변환 함수 필요
- **레이아웃 알고리즘 부재**: random position이라 노드가 겹침. fitView로 화면 안에 들어오긴 하지만 보기 안 좋음. dagre 또는 elk 도입 권장 (Phase 1) — `pnpm add dagre @types/dagre`
- **큰 그래프 성능**: 노드 수백 개 이상이면 React Flow도 느려짐. depth=1로 제한, 사용자가 명시적으로 expand해야 추가 fetch
- **노드 타입별 시각적 구분 부재**: 모든 노드가 default로 표시 → session/project/agent 구분 안 됨. 색상/아이콘 구분 필요 (Phase 1 또는 본 task에서 간단히 추가)
- **fetch 중복**: 같은 노드 expand 시 중복 호출 방지를 expandedSet으로 처리 — 동작 검증 필요
- **state 동기화**: graph state는 컴포넌트 unmount 시 사라짐. 오버레이 닫고 다시 열면 처음부터. 의도된 동작 (오래된 그래프 재사용 안 함)
- **xyflow attribution**: free 버전에 attribution 표시. `proOptions: { hideAttribution: true }`는 Pro 라이선스 필요할 수 있음 — 라이선스 확인 후 표시 유지 결정. AGPL 호환성도 확인

## Scope boundary

수정 금지:
- `crates/` 전체 — 백엔드 변경 금지
- `web/src/routes/SessionsRoute.tsx`, `SessionDetailRoute.tsx`, `DailyRoute.tsx`, `WikiRoute.tsx` — Task 06, 07에서 완료
- `web/src/components/SessionHeader.tsx`, `TagEditor.tsx`, `FavoriteButton.tsx` — Task 07
- `.github/workflows/`, `README.md` — Task 09
