import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";

/**
 * Obsidian-style force-directed graph (Stage 5 — option B).
 *
 * - d3-force-simulation 으로 layout, SVG 직접 그리기
 * - 노드 hover 시 인접 노드/엣지 강조, 나머지 dim
 * - 노드 타입별 색 (project = brand / topic = info / session = text-3)
 * - 노드 크기 = degree 기반 (radius 2.8 ~ 11)
 * - 가운데 vignette gradient (canvas 가장자리 페이드)
 *
 * 데이터 형식 (production graph_query 결과를 이 shape 로 매핑):
 *   nodes: [{ id, type, label? }]
 *   edges: [{ source, target }]
 *
 * 사용자 클릭:
 *   - session 노드 → `onSessionClick(id)` 발화 (호출자가 라우팅 결정)
 *   - 그 외 → expand 가 의도되지만 본 컴포넌트는 단일 fetch 결과만 시각화 (확장은 미래)
 */
interface NodeData {
  id: string;
  type: string; // "project" | "topic" | "session" | "agent" | "tool" | ...
  label?: string;
}

interface EdgeData {
  source: string;
  target: string;
}

interface SimNode extends SimulationNodeDatum {
  id: string;
  type: string;
  label?: string;
}

type SimLink = SimulationLinkDatum<SimNode>;

interface Props {
  nodes: NodeData[];
  edges: EdgeData[];
  /** 표시 안 할 노드 타입 set (예: {"session"} 면 session 만 숨김). 비면 전체 표시. */
  hiddenTypes?: ReadonlySet<string>;
  onSessionClick?: (id: string) => void;
}

const NODE_FILL: Record<string, string> = {
  project: "var(--accent)",
  topic: "var(--info)",
  agent: "var(--success)",
  tool: "var(--warn)",
  session: "var(--text-3)",
};

function fill(type: string): string {
  return NODE_FILL[type] ?? "var(--text-3)";
}

const BASE_R: Record<string, number> = {
  project: 7,
  topic: 4.5,
  agent: 5,
  tool: 4,
  session: 2.8,
};

function baseRadius(type: string): number {
  return BASE_R[type] ?? 3;
}

export function ObsidianGraph({ nodes, edges, hiddenTypes, onSessionClick }: Props) {
  // 필터링된 노드/엣지 — 양쪽 노드가 visible 한 엣지만 통과.
  const filtered = useMemo(() => {
    if (!hiddenTypes || hiddenTypes.size === 0) return { nodes, edges };
    const visibleNodes = nodes.filter((n) => !hiddenTypes.has(n.type));
    const visibleIds = new Set(visibleNodes.map((n) => n.id));
    const visibleEdges = edges.filter(
      (e) => visibleIds.has(e.source) && visibleIds.has(e.target),
    );
    return { nodes: visibleNodes, edges: visibleEdges };
  }, [nodes, edges, hiddenTypes]);
  // 이후 코드는 filtered.nodes / filtered.edges 사용.
  const fNodes = filtered.nodes;
  const fEdges = filtered.edges;
  // 아래 기존 nodes/edges 참조를 이 alias 가 그대로 받도록 사실상 대체.
  const wrapperRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<string | null>(null);
  const [size, setSize] = useState({ w: 800, h: 600 });

  // 컨테이너 크기 측정 (responsive)
  useEffect(() => {
    if (!wrapperRef.current) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0].contentRect;
      setSize({ w: Math.max(320, r.width), h: Math.max(320, r.height) });
    });
    ro.observe(wrapperRef.current);
    return () => ro.disconnect();
  }, []);

  // adjacency for hover + degree
  const { adjacency, degree } = useMemo(() => {
    const adj = new Map<string, Set<string>>();
    const deg = new Map<string, number>();
    for (const e of fEdges) {
      if (!adj.has(e.source)) adj.set(e.source, new Set());
      if (!adj.has(e.target)) adj.set(e.target, new Set());
      adj.get(e.source)!.add(e.target);
      adj.get(e.target)!.add(e.source);
      deg.set(e.source, (deg.get(e.source) ?? 0) + 1);
      deg.set(e.target, (deg.get(e.target) ?? 0) + 1);
    }
    return { adjacency: adj, degree: deg };
  }, [fEdges]);

  // 노드/링크 구성만 (좌표는 랜덤 시작 — 실제 배치는 아래 live 시뮬이 애니메이션).
  // 링크 필터는 id Set 으로 O(E) (이전 some×some = O(E×N) 제거).
  const positioned = useMemo(() => {
    if (fNodes.length === 0) return { nodes: [] as SimNode[], edges: [] as SimLink[] };
    // 고립/저차수 topic 노드 제외 — degree < 2 인 topic 은 레이아웃에 거의 기여하지 않으면서
    // 매 tick SVG+라벨 렌더 비용만 유발한다(5000+ → 수백). session/project/agent/tool 은 유지.
    const visible = fNodes.filter(
      (n) => n.type !== "topic" || (degree.get(n.id) ?? 0) >= 2,
    );
    const simNodes: SimNode[] = visible.map((n) => ({
      id: n.id,
      type: n.type,
      label: n.label,
      x: (Math.random() - 0.5) * 100,
      y: (Math.random() - 0.5) * 100,
    }));
    const idSet = new Set(simNodes.map((n) => n.id));
    const simLinks: SimLink[] = fEdges
      .filter((e) => idSet.has(e.source) && idSet.has(e.target))
      .map((e) => ({ source: e.source, target: e.target }));
    return { nodes: simNodes, edges: simLinks };
  }, [fNodes, fEdges, degree]);

  // live d3-force 시뮬레이션 — 메인스레드 동기 300-tick(freeze) 대신 rAF 애니메이션.
  // 노드가 랜덤 시작점에서 퍼지며 순차 등장(Obsidian 거동). alpha 가 식으면 스스로 정지.
  const [tick, bumpTick] = useReducer((c: number) => c + 1, 0);
  useEffect(() => {
    if (positioned.nodes.length === 0) return;
    const sim: Simulation<SimNode, SimLink> = forceSimulation(positioned.nodes)
      // 조기 수렴: alpha 를 빨리 식혀 ~300 tick → ~60 tick 에 정지. 매 tick 재렌더 횟수와
      // 탭 이탈 시 시뮬이 아직 도는 확률을 줄인다.
      .alphaMin(0.05)
      .alphaDecay(0.06)
      .force(
        "link",
        forceLink<SimNode, SimLink>(positioned.edges)
          .id((d) => d.id)
          .distance(70)
          .strength(0.35),
      )
      .force("charge", forceManyBody().strength(-180))
      .force("center", forceCenter(0, 0).strength(0.06))
      .force("collide", forceCollide<SimNode>().radius((d) => baseRadius(d.type) + 6))
      .on("tick", bumpTick);
    // 필터 토글 등으로 positioned 가 새로 만들어지면 이전 시뮬 정지(타이머 누수 방지).
    return () => {
      sim.stop();
    };
  }, [positioned]);

  const radius = (n: SimNode): number => {
    const d = degree.get(n.id) ?? 0;
    return baseRadius(n.type) + Math.min(7, d * 0.55);
  };

  // viewBox: 노드 좌표 bounding box + padding
  const vb = useMemo(() => {
    if (positioned.nodes.length === 0) return { x: -200, y: -150, w: 400, h: 300 };
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const n of positioned.nodes) {
      const x = n.x ?? 0;
      const y = n.y ?? 0;
      if (x < minX) minX = x;
      if (x > maxX) maxX = x;
      if (y < minY) minY = y;
      if (y > maxY) maxY = y;
    }
    const pad = 60;
    return {
      x: minX - pad,
      y: minY - pad,
      w: Math.max(200, maxX - minX + pad * 2),
      h: Math.max(200, maxY - minY + pad * 2),
    };
    // tick 을 deps 에 포함 — 시뮬 애니메이션 중 노드가 움직이면 viewBox 도 따라 갱신.
  }, [positioned.nodes, tick]);

  const isLit = (id: string): boolean =>
    !!hover && (id === hover || (adjacency.get(hover)?.has(id) ?? false));
  const isDim = (id: string): boolean => !!hover && !isLit(id);

  return (
    <div
      ref={wrapperRef}
      className="relative h-full w-full overflow-hidden"
      style={{ background: "var(--bg)" }}
    >
      <svg
        width={size.w}
        height={size.h}
        viewBox={`${vb.x} ${vb.y} ${vb.w} ${vb.h}`}
        preserveAspectRatio="xMidYMid meet"
        className="block"
      >
        <defs>
          <radialGradient id="og-vignette" cx="50%" cy="50%" r="62%">
            <stop offset="0%" stopColor="rgba(0,0,0,0)" />
            <stop offset="80%" stopColor="rgba(0,0,0,0)" />
            <stop offset="100%" stopColor="rgba(0,0,0,.18)" />
          </radialGradient>
          <filter id="og-glow" x="-200%" y="-200%" width="500%" height="500%">
            <feGaussianBlur stdDeviation="3" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* edges */}
        <g>
          {positioned.edges.map((e, i) => {
            const a = e.source as SimNode;
            const b = e.target as SimNode;
            if (typeof a !== "object" || typeof b !== "object") return null;
            const lit = !!hover && (a.id === hover || b.id === hover);
            const dim = !!hover && !lit;
            return (
              <line
                key={i}
                x1={a.x ?? 0}
                y1={a.y ?? 0}
                x2={b.x ?? 0}
                y2={b.y ?? 0}
                stroke={lit ? "var(--accent)" : "var(--border)"}
                strokeOpacity={lit ? 0.85 : dim ? 0.18 : 0.55}
                strokeWidth={lit ? 1.2 : 0.7}
              />
            );
          })}
        </g>

        {/* nodes */}
        <g>
          {positioned.nodes.map((n) => {
            const r = radius(n);
            const lit = isLit(n.id) || n.id === hover;
            const dim = isDim(n.id);
            const opacity = dim ? 0.28 : 1;
            const f = fill(n.type);
            const handleClick = () => {
              if (n.type === "session" && onSessionClick) {
                const sid = n.id.startsWith("session:")
                  ? n.id.slice("session:".length)
                  : n.id;
                onSessionClick(sid);
              }
            };
            return (
              <g
                key={n.id}
                transform={`translate(${n.x ?? 0},${n.y ?? 0})`}
                style={{ opacity, cursor: n.type === "session" ? "pointer" : "default" }}
                onMouseEnter={() => setHover(n.id)}
                onMouseLeave={() => setHover((h) => (h === n.id ? null : h))}
                onClick={handleClick}
              >
                {lit && (
                  <circle r={r + 6} fill={f} opacity="0.18" filter="url(#og-glow)" />
                )}
                <circle
                  r={r}
                  fill={f}
                  stroke={n.type === "project" ? "var(--bg)" : "transparent"}
                  strokeWidth={n.type === "project" ? 1.5 : 0}
                />
                {(n.type !== "session" || lit) && (
                  <text
                    y={r + 11}
                    textAnchor="middle"
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: n.type === "project" ? 10 : 9,
                      fill: "var(--text-3)",
                      pointerEvents: "none",
                      opacity: dim ? 0.4 : n.type === "session" && !lit ? 0.7 : 1,
                    }}
                  >
                    {n.label ?? n.id}
                  </text>
                )}
              </g>
            );
          })}
        </g>

        <rect
          x={vb.x}
          y={vb.y}
          width={vb.w}
          height={vb.h}
          fill="url(#og-vignette)"
          pointerEvents="none"
        />
      </svg>

      {/* Hover info bar (좌하단, prototype 의 graph__panel hover info 의 축약) */}
      {hover && (
        <div className="absolute left-ds-3 bottom-ds-3 px-ds-3 py-ds-2 rounded-md border border-hairline bg-[var(--surface)] shadow-ds-2 text-t-meta text-text-2 pointer-events-none">
          <span className="font-mono text-text-3 mr-ds-2">{nodeTypeLabel(hover, positioned.nodes)}</span>
          <span>{nodeLabel(hover, positioned.nodes)}</span>
          <span className="ml-ds-2 font-mono text-text-4">· {degree.get(hover) ?? 0} links</span>
        </div>
      )}
    </div>
  );
}

function nodeLabel(id: string, nodes: SimNode[]): string {
  return nodes.find((n) => n.id === id)?.label ?? id;
}

function nodeTypeLabel(id: string, nodes: SimNode[]): string {
  return nodes.find((n) => n.id === id)?.type ?? "?";
}
