import { useMemo } from "react";
import { useNavigate, useParams } from "react-router";
import { Loader2 } from "lucide-react";
import {
  useWikiList,
  useWikiPage,
  useWikiSearch,
  type WikiSearchMode,
} from "@/hooks/useWiki";
import { MarkdownView } from "@/components/MarkdownView";
import { useUi } from "@/lib/store";

/**
 * 위키 라우트.
 *
 * - 좌측: vault/wiki/projects/*.md 실존 페이지 리스트 (`useWikiList`).
 * - 검색은 TopNav 의 HeaderSearch 가 store 에 lift (mode: keyword/semantic/hybrid).
 *   검색어가 있으면 좌측을 useWikiSearch 결과로 대체. semantic/hybrid 는 P40 기반 +
 *   Ollama 미실행 시 backend 가 keyword 로 자동 fallback.
 * - 우측: `GET /api/wiki/{project}` 본문 (`useWikiPage`).
 */
export default function WikiRoute() {
  const { project } = useParams<{ project?: string }>();
  const navigate = useNavigate();

  const query = useUi((s) => s.query);
  const globalMode = useUi((s) => s.searchMode);
  // store 의 mode 가 wiki 가 지원하는 3 mode 중 하나로 매핑.
  const mode: WikiSearchMode = globalMode as WikiSearchMode;
  const trimmedQuery = query.trim();

  const projectsQuery = useWikiList();
  const searchQuery = useWikiSearch(trimmedQuery, {
    mode,
    limit: 20,
    enabled: trimmedQuery.length > 0,
  });
  const wikiQuery = useWikiPage(project);

  // 검색 결과의 path 에서 project 이름(파일 stem) 추출.
  // 예: "wiki/projects/seCall.md" → { project: "seCall", path }
  // projects/ 외 카테고리는 클릭 시 /wiki/{project} 가 404 가능 — UI 상 raw path 표시.
  const searchItems = useMemo(() => {
    if (!searchQuery.data) return [];
    return searchQuery.data.results.map((r) => {
      const stem = r.path.replace(/^.*\//, "").replace(/\.md$/, "");
      const isProjectPage = r.path.includes("/projects/");
      return {
        path: r.path,
        title: r.title || stem,
        preview: r.preview,
        navProject: isProjectPage ? stem : null, // null 이면 클릭 비활성
        updated: r.updated,
      };
    });
  }, [searchQuery.data]);

  const isSearching = trimmedQuery.length > 0;

  return (
    <div className="grid grid-cols-[var(--list-w)_1fr] h-full">
      <aside className="border-r border-hairline bg-[var(--surface)] overflow-auto flex flex-col">
        {/* 헤더 (검색은 TopNav 의 HeaderSearch 가 처리) */}
        <div className="px-ds-3 py-ds-2 eyebrow border-b border-hairline">
          {isSearching
            ? `검색 결과 (${searchQuery.data?.count ?? 0})`
            : "Projects"}
        </div>

        {/* 본문: 검색 모드 vs 전체 리스트 */}
        {isSearching ? (
          <div className="flex-1 overflow-auto">
            {searchQuery.isLoading && (
              <div className="p-3 text-xs text-text-3 flex items-center">
                <Loader2 className="size-3 animate-spin mr-2" /> 검색 중…
              </div>
            )}
            {searchQuery.error && (
              <div className="p-3 text-xs text-status-danger">
                {searchQuery.error instanceof Error
                  ? searchQuery.error.message
                  : String(searchQuery.error)}
              </div>
            )}
            <div className="divide-y divide-border">
              {searchItems.map((item) => (
                <button
                  key={item.path}
                  type="button"
                  disabled={!item.navProject}
                  onClick={() =>
                    item.navProject &&
                    navigate(`/wiki/${encodeURIComponent(item.navProject)}`)
                  }
                  className={`block w-full text-left px-3 py-2 text-sm transition-colors ${
                    item.navProject
                      ? "hover:bg-accent cursor-pointer"
                      : "opacity-60 cursor-not-allowed"
                  } ${
                    item.navProject === project ? "bg-accent font-medium" : ""
                  }`}
                >
                  <div>{item.title}</div>
                  <div className="text-[10px] text-text-3 mt-0.5 font-mono">
                    {item.path}
                  </div>
                  {item.preview && (
                    <div className="text-[11px] text-text-3 mt-1 line-clamp-2">
                      {item.preview}
                    </div>
                  )}
                </button>
              ))}
            </div>
            {searchQuery.data && searchQuery.data.results.length === 0 && (
              <div className="p-3 text-xs text-text-3 italic">
                결과가 없습니다
              </div>
            )}
          </div>
        ) : (
          <div className="flex-1 overflow-auto">
            {projectsQuery.isLoading && (
              <div className="p-3 text-xs text-text-3 flex items-center">
                <Loader2 className="size-3 animate-spin mr-2" /> 불러오는 중…
              </div>
            )}
            {projectsQuery.error && (
              <div className="p-3 text-xs text-status-danger">
                {projectsQuery.error instanceof Error
                  ? projectsQuery.error.message
                  : String(projectsQuery.error)}
              </div>
            )}
            <div className="divide-y divide-border">
              {projectsQuery.data?.projects.map((item) => (
                <button
                  key={item.project}
                  type="button"
                  onClick={() =>
                    navigate(`/wiki/${encodeURIComponent(item.project)}`)
                  }
                  className={`block w-full text-left px-3 py-2 text-sm hover:bg-accent transition-colors ${
                    item.project === project ? "bg-accent font-medium" : ""
                  }`}
                >
                  <div>{item.project}</div>
                  {item.updated && (
                    <div className="text-[10px] text-text-3 mt-0.5">
                      {item.updated.slice(0, 10)}
                    </div>
                  )}
                </button>
              ))}
            </div>
            {projectsQuery.data && projectsQuery.data.projects.length === 0 && (
              <div className="p-3 text-xs text-text-3 italic">
                위키 페이지가 없습니다 (vault/wiki/projects/*.md)
              </div>
            )}
          </div>
        )}
      </aside>

      <div className="overflow-auto p-6 max-w-4xl">
        {!project && (
          <div className="text-text-3 text-sm">
            좌측에서 프로젝트를 선택하세요
          </div>
        )}
        {project && wikiQuery.isLoading && (
          <div className="flex items-center text-text-3 text-sm">
            <Loader2 className="size-4 animate-spin mr-2" /> 불러오는 중…
          </div>
        )}
        {project && wikiQuery.error && (
          <div className="text-status-danger text-sm">
            위키 페이지를 찾을 수 없습니다:{" "}
            <span className="font-mono">
              {wikiQuery.error instanceof Error
                ? wikiQuery.error.message
                : String(wikiQuery.error)}
            </span>
            <div className="mt-2 text-xs text-text-3 italic">
              <code className="font-mono">vault/wiki/projects/</code> 아래 해당 프로젝트의 페이지가 없을 수 있습니다.
            </div>
          </div>
        )}
        {project && wikiQuery.data && (
          <article>
            <header className="mb-6 pb-3 border-b border-border">
              <h1 className="text-2xl font-semibold">{wikiQuery.data.project}</h1>
              <div className="text-[11px] text-text-3 mt-1 flex flex-wrap gap-x-3">
                <span className="font-mono opacity-70">{wikiQuery.data.path}</span>
                {wikiQuery.data.updated && (
                  <span>last modified: {wikiQuery.data.updated}</span>
                )}
              </div>
            </header>
            <MarkdownView content={wikiQuery.data.content} />
          </article>
        )}
      </div>
    </div>
  );
}
