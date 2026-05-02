import { Star } from "lucide-react";
import { useMemo } from "react";
import { highlightTerms, tokenizeQuery } from "@/lib/highlight";
import { tagColor } from "@/lib/tagColor";
import type { SessionListItem as Session } from "@/lib/types";

interface Props {
  session: Session;
  /** 검색어 (P34 Task 02). summary 안의 매칭 토큰을 하이라이트한다. */
  query?: string;
  selected?: boolean;
  onSelect: () => void;
}

const MAX_TAGS = 4;

export function SessionListItem({
  session,
  query,
  selected = false,
  onSelect,
}: Props) {
  const heading = session.project ?? session.agent;
  const subheading = session.project ? session.agent : null;
  const tags = session.tags.slice(0, MAX_TAGS);
  const hidden = Math.max(0, session.tags.length - tags.length);
  const terms = useMemo(() => tokenizeQuery(query ?? ""), [query]);

  return (
    <button
      type="button"
      onClick={onSelect}
      className={`w-full text-left px-3 py-2.5 transition-colors block border-l-2 ${
        selected
          ? "border-l-primary bg-accent/40"
          : "border-l-transparent hover:bg-accent/20"
      }`}
    >
      <div className="flex items-baseline justify-between gap-2 mb-1">
        <div className="flex items-baseline gap-2 min-w-0">
          <span className="text-sm font-medium truncate">{heading}</span>
          {subheading && (
            <span className="text-xs text-muted-foreground truncate">{subheading}</span>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {session.is_favorite && (
            <Star className="size-3 fill-amber-400 text-amber-400" />
          )}
          <span className="text-xs text-muted-foreground tabular-nums">{session.date}</span>
        </div>
      </div>
      <p className="text-xs text-muted-foreground line-clamp-2 mb-1.5">
        {session.summary ? (
          terms.length > 0 ? (
            highlightTerms(session.summary, terms)
          ) : (
            session.summary
          )
        ) : (
          <span className="italic">요약 없음</span>
        )}
      </p>
      {(tags.length > 0 || session.turn_count > 0) && (
        <div className="flex items-center flex-wrap gap-1">
          {tags.map((tag) => (
            <span
              key={tag}
              className={`text-[10px] px-1.5 py-0.5 rounded ring-1 ring-inset ${tagColor(tag)}`}
            >
              {tag}
            </span>
          ))}
          {hidden > 0 && (
            <span className="text-[10px] text-muted-foreground">+{hidden}</span>
          )}
          <span className="text-[10px] text-muted-foreground ml-auto">
            {session.turn_count} turns
          </span>
        </div>
      )}
    </button>
  );
}
