import { Star, X } from "lucide-react";
import { useMemo } from "react";
import { AgentDot } from "@/components/AgentDot";
import { highlightTerms, tokenizeQuery } from "@/lib/highlight";
import type { SessionListItem as Session } from "@/lib/types";

/**
 * 세션 리스트 1개 row — prototype route-sessions.jsx 의 SessionListItem (sli) 패턴 (Stage 2c).
 *
 * head: AgentDot + project mono + when + fav star
 * title: 1줄 medium weight
 * summary: line-clamp-2, muted
 * foot: turns mono + 작은 tag chips (옵셔널)
 *
 * 선택 시 좌측 brand 색 rail (border-l-2) + soft 배경.
 */
interface Props {
  session: Session;
  query?: string;
  selected?: boolean;
  onSelect: () => void;
  /** 지정 시 우측 상단에 삭제(X) 버튼 노출. 클릭하면 호출된다. */
  onDelete?: () => void;
}

const MAX_TAGS = 3;

export function SessionListItem({
  session,
  query,
  selected = false,
  onSelect,
  onDelete,
}: Props) {
  const tags = session.tags.slice(0, MAX_TAGS);
  const hidden = Math.max(0, session.tags.length - tags.length);
  const terms = useMemo(() => tokenizeQuery(query ?? ""), [query]);

  const project = session.project ?? null;

  return (
    <div className="relative group border-b border-hairline">
      <button
      type="button"
      onClick={onSelect}
      className={[
        "w-full text-left block px-ds-4 py-ds-3 border-l-2 transition-colors duration-base ease-ds",
        selected
          ? "border-l-brand bg-brand-soft"
          : "border-l-transparent hover:bg-surface-2 hover:border-l-hairline",
      ].join(" ")}
    >
      {/* head */}
      <div className="flex items-center gap-ds-2 text-t-meta text-text-3 mb-ds-1">
        <AgentDot agent={session.agent} />
        {project && (
          <span className="font-mono text-t-mono text-text-2 truncate">{project}</span>
        )}
        <span aria-hidden className="text-text-4">·</span>
        <span className="tabular-nums truncate">{session.date}</span>
        <span className="flex-1" />
        {session.is_favorite && (
          <Star className="size-3 fill-status-warn text-status-warn" />
        )}
      </div>

      {/* title (project 가 없을 땐 agent, 있을 땐 summary 의 첫 줄을 굵게 보여주는 대신 agent 만 보조표기) */}
      {!project && (
        <div className="text-t-body font-medium text-text mb-ds-1">
          {session.agent}
        </div>
      )}

      {/* summary */}
      <p className="text-t-small text-text-2 line-clamp-2 mb-ds-1">
        {session.summary ? (
          terms.length > 0 ? (
            highlightTerms(session.summary, terms)
          ) : (
            session.summary
          )
        ) : (
          <span className="italic text-text-4">요약 없음</span>
        )}
      </p>

      {/* foot */}
      {(tags.length > 0 || session.turn_count > 0) && (
        <div className="flex items-center gap-ds-2 text-t-meta text-text-3">
          {session.turn_count > 0 && (
            <span className="font-mono text-t-mono tabular-nums">
              {session.turn_count} turns
            </span>
          )}
          {tags.length > 0 && (
            <>
              {session.turn_count > 0 && <span aria-hidden className="text-text-4">·</span>}
              <div className="flex items-center gap-ds-1 min-w-0 overflow-hidden">
                {tags.map((tag) => (
                  <span
                    key={tag}
                    className="inline-flex items-center text-t-caption px-1.5 py-0.5 rounded-sm bg-surface-3 text-text-2"
                  >
                    <span className="opacity-60 mr-0.5">#</span>
                    {tag}
                  </span>
                ))}
                {hidden > 0 && (
                  <span className="text-t-caption text-text-4">+{hidden}</span>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </button>
      {onDelete && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          aria-label="세션 삭제"
          className="absolute right-ds-2 top-ds-2 flex size-6 items-center justify-center rounded-md text-text-4 opacity-0 transition-opacity duration-fast ease-ds hover:bg-surface-3 hover:text-status-danger focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-status-danger group-hover:opacity-100"
        >
          <X className="size-3.5" />
        </button>
      )}
    </div>
  );
}
