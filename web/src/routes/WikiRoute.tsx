import { useNavigate, useParams } from "react-router";
import { Loader2 } from "lucide-react";
import { useProjects } from "@/hooks/useSessions";
import { useWikiPage } from "@/hooks/useWiki";
import { MarkdownView } from "@/components/MarkdownView";

/**
 * 위키 라우트.
 *
 * Phase 1 (P33 Task 04)부터 `GET /api/wiki/{project}`로 위키 페이지 본문 전체를
 * 가져와 마크다운으로 렌더한다. 좌측에는 세션 DB의 `/api/projects` 리스트를 띄우고,
 * 클릭 시 해당 프로젝트의 wiki 페이지(vault/wiki/projects/{safe_name}.md)를 본다.
 *
 * 페이지가 없으면 404 → 안내 메시지 표시.
 */
export default function WikiRoute() {
  const { project } = useParams<{ project?: string }>();
  const navigate = useNavigate();

  const projectsQuery = useProjects();
  const wikiQuery = useWikiPage(project);

  return (
    <div className="grid grid-cols-[260px_1fr] h-full">
      <aside className="border-r border-border overflow-auto">
        <div className="p-3 text-xs text-muted-foreground uppercase tracking-wide border-b border-border">
          Projects
        </div>
        {projectsQuery.isLoading && (
          <div className="p-3 text-xs text-muted-foreground flex items-center">
            <Loader2 className="size-3 animate-spin mr-2" /> 불러오는 중…
          </div>
        )}
        {projectsQuery.error && (
          <div className="p-3 text-xs text-rose-400">
            {projectsQuery.error instanceof Error
              ? projectsQuery.error.message
              : String(projectsQuery.error)}
          </div>
        )}
        <div className="divide-y divide-border">
          {projectsQuery.data?.projects.map((p) => (
            <button
              key={p}
              type="button"
              onClick={() => navigate(`/wiki/${encodeURIComponent(p)}`)}
              className={`block w-full text-left px-3 py-2 text-sm hover:bg-accent transition-colors ${
                p === project ? "bg-accent font-medium" : ""
              }`}
            >
              {p}
            </button>
          ))}
        </div>
        {projectsQuery.data && projectsQuery.data.projects.length === 0 && (
          <div className="p-3 text-xs text-muted-foreground italic">
            프로젝트가 없습니다
          </div>
        )}
      </aside>

      <div className="overflow-auto p-6 max-w-4xl">
        {!project && (
          <div className="text-muted-foreground text-sm">
            좌측에서 프로젝트를 선택하세요
          </div>
        )}
        {project && wikiQuery.isLoading && (
          <div className="flex items-center text-muted-foreground text-sm">
            <Loader2 className="size-4 animate-spin mr-2" /> 불러오는 중…
          </div>
        )}
        {project && wikiQuery.error && (
          <div className="text-rose-400 text-sm">
            위키 페이지를 찾을 수 없습니다:{" "}
            <span className="font-mono">
              {wikiQuery.error instanceof Error
                ? wikiQuery.error.message
                : String(wikiQuery.error)}
            </span>
            <div className="mt-2 text-xs text-muted-foreground italic">
              <code className="font-mono">vault/wiki/projects/</code> 아래 해당 프로젝트의 페이지가 없을 수 있습니다.
            </div>
          </div>
        )}
        {project && wikiQuery.data && (
          <article>
            <header className="mb-6 pb-3 border-b border-border">
              <h1 className="text-2xl font-semibold">{wikiQuery.data.project}</h1>
              <div className="text-[11px] text-muted-foreground mt-1 flex flex-wrap gap-x-3">
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
