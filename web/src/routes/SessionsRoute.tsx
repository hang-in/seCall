import { useRef, useState } from "react";
import { Outlet } from "react-router";
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

  // wiki 모드(`hybrid`)가 store 에 남아 있으면 sessions 에선 keyword 로 폴백.
  const mode: SearchMode =
    globalMode === "hybrid" ? "keyword" : (globalMode as SearchMode);

  const outletContext: SessionsOutletContext = { query, mode };

  return (
    <div className="grid grid-cols-[var(--list-w)_1fr] h-full">
      <div className="border-r border-hairline bg-[var(--surface)] flex flex-col overflow-hidden min-h-0">
        <div ref={scrollRef} className="flex-1 overflow-auto">
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

      <div className="overflow-auto min-w-0 bg-[var(--bg)]">
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
