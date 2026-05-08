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
      <div className="mt-8 text-xs text-text-3">
        관련 세션 로딩...
      </div>
    );
  }
  if (!items.length) return null;

  return (
    <section className="mt-ds-8 border-t border-hairline pt-ds-4">
      <h3 className="eyebrow mb-ds-3 flex items-center gap-ds-2">
        <Network className="size-3.5" /> 관련 세션 ({items.length})
      </h3>
      <ul className="space-y-ds-1">
        {items.map((it) => (
          <li key={it.id}>
            <button
              type="button"
              onClick={() =>
                navigate(`/sessions/${encodeURIComponent(it.id)}`)
              }
              className="w-full text-left p-ds-2 rounded-md hover:bg-surface-2 text-t-small flex items-center justify-between gap-ds-2 transition-colors duration-fast ease-ds"
            >
              <span className="truncate text-text-2">{it.title ?? it.id.slice(0, 8)}</span>
              <span className="text-t-meta text-text-3 tabular-nums shrink-0">
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
