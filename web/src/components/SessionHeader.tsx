import type { SessionDetail } from "@/lib/types";
import { TagEditor } from "./TagEditor";
import { FavoriteButton } from "./FavoriteButton";
import { RoleStackedBar, ToolUseList } from "./MiniChart";
import { NoteEditor } from "./NoteEditor";

interface Props {
  /** URL에서 가져온 세션 ID — fallback (detail.id가 정상이면 그것 우선). */
  id: string;
  /**
   * `/api/get` (full=true) 응답 — agent/model/project/date/session_type/content +
   * P32 Task 06 rework로 추가된 id/tags/is_favorite/turn_count/start_time/summary.
   */
  detail: SessionDetail;
}

/**
 * 세션 본문 상단 헤더.
 *
 * 레이아웃:
 *   1행: 제목(project ?? agent) + 우상단 즐겨찾기 토글
 *   2행: 메타 정보 (date · start_time · turn_count · agent · model · session_type · id 단축)
 *   3행: 태그 편집기
 *
 * P32 Task 06 rework: 세션 메타를 sessions 리스트 캐시가 아닌 `/api/get` 응답에서
 * 직접 사용. `/daily`, `/wiki`, 그래프 오버레이에서 직접 진입 시에도 정확한
 * 태그/즐겨찾기 표시 보장.
 */
export function SessionHeader({ id, detail }: Props) {
  const sessionId = detail.id ?? id;
  const tags = detail.tags ?? [];
  const favorite = detail.is_favorite ?? false;
  const turnCount = detail.turn_count;
  const startTime = detail.start_time;

  return (
    <header className="border-b border-border pb-4 mb-6 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-xl font-semibold truncate">
            {detail.project ?? detail.agent}
          </h1>
          <div className="text-xs text-muted-foreground mt-1 flex flex-wrap items-center gap-x-3 gap-y-0.5">
            <span className="tabular-nums">{detail.date}</span>
            {startTime && <span className="tabular-nums">{startTime}</span>}
            {typeof turnCount === "number" && <span>{turnCount} turns</span>}
            <span>agent: {detail.agent}</span>
            {detail.model && <span>model: {detail.model}</span>}
            <span>type: {detail.session_type || "-"}</span>
            <span className="font-mono opacity-60">id: {sessionId.slice(0, 8)}</span>
          </div>
        </div>
        <FavoriteButton sessionId={sessionId} initial={favorite} />
      </div>
      <TagEditor sessionId={sessionId} initial={tags} />
      <NoteEditor sessionId={sessionId} initial={detail.notes} />
      {detail.turn_role_counts && (
        <RoleStackedBar {...detail.turn_role_counts} />
      )}
      {detail.tool_use_counts && detail.tool_use_counts.length > 0 && (
        <details className="text-xs">
          <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
            Tool 사용 ({detail.tool_use_counts.length})
          </summary>
          <div className="mt-1.5">
            <ToolUseList tools={detail.tool_use_counts} />
          </div>
        </details>
      )}
    </header>
  );
}
