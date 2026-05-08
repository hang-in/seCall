import { useNavigate } from "react-router";
import { GraphCanvas } from "@/components/GraphCanvas";

/**
 * `/graph` 라우트 — 전체 화면 그래프 (react-flow).
 * 시작 노드/expand 동작은 GraphCanvas 내부 (sessions overlay 와 같은 컴포넌트 재사용).
 *
 * session 노드를 클릭하면 해당 세션 상세로 이동, 그 외 타입은 expand 만 (GraphCanvas 내부).
 */
export default function GraphRoute() {
  const navigate = useNavigate();

  const handleNodeClick = (nodeId: string, nodeType: string) => {
    if (nodeType === "session") {
      // node id 형식 "session:UUID" → UUID 부분만 추출.
      const sid = nodeId.startsWith("session:")
        ? nodeId.slice("session:".length)
        : nodeId;
      navigate(`/sessions/${encodeURIComponent(sid)}`);
    }
  };

  return (
    <div className="h-full w-full bg-[var(--bg)]">
      <GraphCanvas onNodeClick={handleNodeClick} />
    </div>
  );
}
