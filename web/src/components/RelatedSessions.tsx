import { Network } from "lucide-react";
import { useNavigate } from "react-router";
import { useRelated } from "@/hooks/useRelated";

/**
 * 세션 상세 화면 하단의 "관련 세션" 패널 (P34 Task 05).
 *
 * - 그래프 인접 / 같은 프로젝트 / 같은 태그 세 source를 dedup하여 최대 10개 표시
 * - 항목이 0개면 `null`을 반환해 빈 섹션이 렌더되지 않도록 한다
 * - 클릭 시 해당 세션 상세로 이동
 */
export function RelatedSessions({ sessionId }: { sessionId: string }) {
  const { items, isLoading } = useRelated(sessionId);
  const navigate = useNavigate();

  if (isLoading) {
    return (
      <div className="mt-8 text-xs text-muted-foreground">
        관련 세션 로딩...
      </div>
    );
  }
  if (!items.length) return null;

  return (
    <section className="mt-8 border-t border-border pt-4">
      <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
        <Network className="size-4" /> 관련 세션 ({items.length})
      </h3>
      <ul className="space-y-1.5">
        {items.map((it) => (
          <li key={it.id}>
            <button
              type="button"
              onClick={() =>
                navigate(`/sessions/${encodeURIComponent(it.id)}`)
              }
              className="w-full text-left p-2 rounded hover:bg-accent text-sm flex items-center justify-between gap-2"
            >
              <span className="truncate">{it.title ?? it.id.slice(0, 8)}</span>
              <span className="text-xs text-muted-foreground tabular-nums shrink-0">
                {it.reason}
                {it.date ? ` · ${it.date}` : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
