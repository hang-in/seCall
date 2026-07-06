import { useRef, useState } from "react";
import { Outlet, useMatch } from "react-router";
import { CollapsibleFilter } from "@/components/CollapsibleFilter";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionList } from "@/components/SessionList";
import { useUi, type GlobalSearchMode } from "@/lib/store";
import type { SearchMode, SessionFilterState } from "@/lib/types";

/**
 * 2-pane 세션 화면.
 * - 검색은 TopNav 의 HeaderSearch 가 store 에 lift (sessions 라우트는 keyword/semantic 두 모드).
 * - 좌측: SessionList (가득) + 하단 접히는 CollapsibleFilter
 * - 우측: 자식 라우트 (Outlet — index 또는 :id)
 */
export default function SessionsRoute() {
  const query = useUi((s) => s.query);
  const globalMode = useUi((s) => s.searchMode);
  const [filters, setFilters] = useState<SessionFilterState>({});
  const scrollRef = useRef<HTMLDivElement>(null);

  // 반응형: 모바일(<md)에서는 단일 컬럼. 세션이 선택되면(:id) 상세가 리스트를
  // 전체 폭으로 덮고, 미선택 시 리스트만 보인다. md+ 에서는 항상 2-pane.
  // (부모 라우트라 useParams 로는 자식 :id 를 못 읽으므로 useMatch 로 감지)
  const hasSelection = Boolean(useMatch("/sessions/:id"));

  // wiki 모드(`hybrid`)가 store 에 남아 있으면 sessions 에선 keyword 로 폴백.
  const mode: SearchMode =
    globalMode === "hybrid" ? "keyword" : (globalMode as SearchMode);

  const outletContext: SessionsOutletContext = { query, mode };

  return (
    <div className="grid h-full min-h-0 grid-cols-1 md:grid-cols-[300px_minmax(0,1fr)] lg:grid-cols-[var(--list-w)_minmax(0,1fr)]">
      {/* 좌: 리스트 (surface 계층). 모바일에서 세션 선택 시 상세가 덮으므로 숨김. */}
      <div
        className={[
          "min-h-0 flex-col overflow-hidden bg-surface md:border-r md:border-hairline",
          hasSelection ? "hidden md:flex" : "flex",
        ].join(" ")}
      >
        <div
          ref={scrollRef}
          className="flex-1 overflow-auto overscroll-contain"
        >
          <SessionList
            query={query}
            mode={mode}
            filters={filters}
            scrollParentRef={scrollRef}
          />
        </div>
        <CollapsibleFilter filters={filters} resultCount={null}>
          <SessionFilters value={filters} onChange={setFilters} />
        </CollapsibleFilter>
      </div>

      {/* 우: 상세/빈 상태 (bg 계층). 모바일에선 선택됐을 때만 전체 폭 노출. */}
      <div
        className={[
          "min-h-0 min-w-0 overflow-auto overscroll-contain bg-[var(--bg)]",
          hasSelection ? "block" : "hidden md:block",
        ].join(" ")}
      >
        <Outlet context={outletContext} />
      </div>
    </div>
  );
}

export interface SessionsOutletContext {
  query: string;
  mode: SearchMode;
}

// Re-export for store consumers
export type { GlobalSearchMode };
