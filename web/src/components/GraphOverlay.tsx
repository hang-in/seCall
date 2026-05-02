import { useEffect, useState } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { useNavigate } from "react-router";
import { useUi } from "@/lib/store";
import { NODE_STYLE_ENTRIES } from "@/lib/graphStyle";
import { GraphCanvas } from "./GraphCanvas";

/**
 * 풀스크린 그래프 오버레이.
 *
 * - useUi.graphOverlayOpen이 true일 때만 렌더링
 * - ESC 또는 X 버튼으로 닫기
 * - 세션 노드 클릭 → /sessions/:id로 navigate + close (자동 폴딩)
 * - 다른 타입 노드 클릭은 GraphCanvas 내부에서 expand만 처리
 * - 좌하단 범례 (collapse 가능) — 8개 노드 타입 색상/아이콘
 */
export function GraphOverlay() {
  const open = useUi((s) => s.graphOverlayOpen);
  const close = useUi((s) => s.toggleGraphOverlay);
  const setSelected = useUi((s) => s.setSelectedSession);
  const navigate = useNavigate();
  const [legendOpen, setLegendOpen] = useState(true);

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
      <div className="absolute top-4 right-4 z-20 flex gap-2">
        <button
          onClick={close}
          className="rounded-md p-2 hover:bg-accent border border-border bg-background"
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
              navigate(`/sessions/${encodeURIComponent(nodeId)}`);
              close();
            }
          }}
        />
      </div>

      {/* 좌하단 범례 — collapse 가능 */}
      <div className="absolute bottom-4 left-4 z-20 rounded-md border border-border bg-background/90 backdrop-blur-sm text-xs shadow-md">
        <button
          onClick={() => setLegendOpen((s) => !s)}
          className="flex items-center gap-1.5 px-3 py-2 w-full hover:bg-accent rounded-md"
          aria-label={legendOpen ? "범례 접기" : "범례 펼치기"}
        >
          {legendOpen ? <ChevronDown className="size-3" /> : <ChevronUp className="size-3" />}
          <span className="font-medium">범례</span>
        </button>
        {legendOpen && (
          <ul className="px-3 pb-2 space-y-1">
            {NODE_STYLE_ENTRIES.map(([key, style]) => {
              const Icon = style.icon;
              return (
                <li key={key} className="flex items-center gap-2">
                  <span
                    className="inline-block size-2 rounded-full"
                    style={{ background: style.color }}
                  />
                  <span style={{ color: style.color }}>
                    <Icon className="size-3" />
                  </span>
                  <span className="text-muted-foreground">{style.label}</span>
                  <span className="text-muted-foreground/60 text-[10px]">({key})</span>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
