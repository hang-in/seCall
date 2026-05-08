import { ArrowLeft } from "lucide-react";
import { useNavigate } from "react-router";
import { AgentDot } from "@/components/AgentDot";
import { FavoriteButton } from "@/components/FavoriteButton";
import { TagEditor } from "@/components/TagEditor";
import type { SessionDetail } from "@/lib/types";

/**
 * 세션 상세 main 영역의 헤더 — prototype route-sessions.jsx 의 pane__head 패턴.
 *
 * 좌상: 뒤로 + crumbs (agent dot + agent · project mono · type)
 * 가운데: 타이틀 (project ?? agent) + meta (id 단축 · 시작 시각)
 * 우측: 즐겨찾기 + (옵션) 추후 액션 버튼
 * 하단: 태그 편집기 1줄
 */
interface Props {
  id: string;
  detail: SessionDetail;
}

export function SessionDetailHead({ id, detail }: Props) {
  const navigate = useNavigate();
  const sessionId = detail.id ?? id;

  return (
    <header className="border-b border-hairline pb-ds-4 mb-ds-6">
      {/* 상단: 뒤로 + crumbs + favorite */}
      <div className="flex items-center gap-ds-3 mb-ds-3 text-t-meta text-text-3">
        <button
          type="button"
          onClick={() => navigate("/sessions")}
          className="inline-flex items-center gap-1 hover:text-text transition-colors duration-fast ease-ds"
        >
          <ArrowLeft className="size-3" /> 리스트
        </button>
        <span aria-hidden className="text-text-4">/</span>
        <AgentDot agent={detail.agent} />
        <span>{detail.agent}</span>
        {detail.project && (
          <>
            <span aria-hidden className="text-text-4">/</span>
            <span className="font-mono text-t-mono text-text-2">{detail.project}</span>
          </>
        )}
        {detail.session_type && detail.session_type !== "interactive" && (
          <>
            <span aria-hidden className="text-text-4">/</span>
            <span className="font-mono text-t-mono">{detail.session_type}</span>
          </>
        )}
        <span className="flex-1" />
        <FavoriteButton sessionId={sessionId} initial={detail.is_favorite ?? false} />
      </div>

      {/* 타이틀 + meta */}
      <h1 className="text-t-display-s font-medium tracking-tight text-text truncate">
        {detail.project ?? detail.agent}
      </h1>
      <div className="mt-ds-1 text-t-meta text-text-3 flex flex-wrap items-center gap-x-ds-3 gap-y-0.5">
        <span className="font-mono opacity-70">{sessionId.slice(0, 8)}</span>
        <span className="tabular-nums">{detail.date}</span>
        {detail.start_time && (
          <span className="tabular-nums">
            {detail.start_time.replace("T", " ").slice(0, 19)}
          </span>
        )}
        {typeof detail.turn_count === "number" && <span>{detail.turn_count} turns</span>}
        {detail.model && <span className="font-mono">model: {detail.model}</span>}
      </div>

      {/* 태그 편집 */}
      <div className="mt-ds-3">
        <TagEditor sessionId={sessionId} initial={detail.tags ?? []} />
      </div>
    </header>
  );
}
