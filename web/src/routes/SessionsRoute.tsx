import { useState } from "react";
import { Outlet } from "react-router";
import { SearchBar } from "@/components/SearchBar";
import { SessionFilters } from "@/components/SessionFilters";
import { SessionList } from "@/components/SessionList";
import type { SearchMode, SessionFilterState } from "@/lib/types";

/**
 * 2-pane 세션 화면.
 * 좌측: 검색 + 필터 + 리스트 (스크롤)
 * 우측: 선택된 세션 상세 (Outlet — index 또는 :id)
 */
export default function SessionsRoute() {
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<SearchMode>("keyword");
  const [filters, setFilters] = useState<SessionFilterState>({});

  // P34 Task 02 — 자식 라우트(SessionDetailRoute)가 검색어/모드를 받아
  // MarkdownView 하이라이트에 사용한다.
  const outletContext: SessionsOutletContext = { query, mode };

  return (
    <div className="grid grid-cols-[400px_1fr] h-full">
      <div className="border-r border-border flex flex-col overflow-hidden min-h-0">
        <div className="p-3 border-b border-border space-y-2 shrink-0">
          <SearchBar
            value={query}
            onChange={setQuery}
            mode={mode}
            onModeChange={setMode}
          />
          <SessionFilters value={filters} onChange={setFilters} />
        </div>
        <div className="flex-1 overflow-auto">
          <SessionList query={query} mode={mode} filters={filters} />
        </div>
      </div>

      <div className="overflow-auto min-w-0">
        <Outlet context={outletContext} />
      </div>
    </div>
  );
}

/**
 * SessionsRoute가 Outlet으로 자식에게 전달하는 컨텍스트.
 * 자식 라우트는 `useOutletContext<SessionsOutletContext>()` 로 읽는다.
 */
export interface SessionsOutletContext {
  query: string;
  mode: SearchMode;
}
