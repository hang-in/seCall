import { useNavigate, useParams, Link } from "react-router";
import { format } from "date-fns";
import { Loader2 } from "lucide-react";
import { useDaily } from "@/hooks/useDaily";
import { DateNavigator } from "@/components/DateNavigator";
import { tagColor } from "@/lib/tagColor";

/**
 * 일일 일기 라우트.
 *
 * `/api/daily` (`do_daily()`)는 마크다운이 아니라 메타 + 프로젝트별 그룹 객체를 반환:
 *   { date, total_sessions, filtered_sessions, topics, projects: { proj: [{session_id,...}] } }
 *
 * UI:
 *   좌측 320px: DateNavigator + 통계 요약
 *   우측: topics 칩 + project별 카드 (각 카드에 세션 리스트, 세션 클릭 시 detail로 이동)
 */
export default function DailyRoute() {
  const { date } = useParams<{ date?: string }>();
  const navigate = useNavigate();
  const today = format(new Date(), "yyyy-MM-dd");
  const current = date ?? today;
  const { data, isLoading, error } = useDaily(current);

  const go = (d: string) => navigate(`/daily/${d}`);

  return (
    <div className="grid grid-cols-[320px_1fr] h-full">
      <aside className="border-r border-hairline bg-[var(--surface)] p-ds-4 space-y-ds-4 overflow-auto">
        <DateNavigator value={current} onChange={go} />
        <div className="text-t-meta text-text-3 space-y-ds-1">
          {data && (
            <>
              <div>
                전체 세션: <span className="text-text">{data.total_sessions}</span>
              </div>
              <div>
                의미있는 세션:{" "}
                <span className="text-text">{data.filtered_sessions}</span>
              </div>
              <div>
                프로젝트:{" "}
                <span className="text-text">
                  {Object.keys(data.projects).length}
                </span>
              </div>
            </>
          )}
        </div>
      </aside>

      <div className="overflow-auto p-ds-6 max-w-5xl bg-[var(--bg)]">
        {isLoading && (
          <div className="flex items-center text-text-3 text-t-small">
            <Loader2 className="size-4 animate-spin mr-ds-2" /> 불러오는 중…
          </div>
        )}
        {error && (
          <div className="text-status-danger text-t-small">
            {error instanceof Error ? error.message : String(error)}
          </div>
        )}
        {data && !isLoading && (
          <div className="space-y-ds-6">
            <header className="space-y-ds-1">
              <h1 className="text-t-display-s font-medium tracking-tight tabular-nums">
                {data.date}
              </h1>
              <div className="text-t-small text-text-3">
                {data.filtered_sessions === 0
                  ? "이 날의 의미있는 세션이 없습니다"
                  : `${data.filtered_sessions}개 세션 / ${Object.keys(data.projects).length}개 프로젝트`}
              </div>
            </header>

            {data.topics.length > 0 && (
              <section>
                <h2 className="eyebrow mb-ds-2">Topics</h2>
                <div className="flex flex-wrap gap-ds-1">
                  {data.topics.map((t) => (
                    <span
                      key={t}
                      className={`text-t-meta px-ds-2 py-0.5 rounded-sm ring-1 ${tagColor(t)}`}
                    >
                      {t}
                    </span>
                  ))}
                </div>
              </section>
            )}

            <section className="space-y-ds-4">
              {Object.entries(data.projects).map(([project, sessions]) => (
                <article
                  key={project}
                  className="border border-hairline rounded-lg p-ds-4 bg-[var(--surface)]"
                >
                  <h3 className="text-t-h2 font-medium mb-ds-3 flex items-baseline gap-ds-2">
                    <span>{project}</span>
                    <span className="text-t-meta text-text-3 font-normal">
                      ({sessions.length})
                    </span>
                  </h3>
                  <ul className="space-y-ds-1">
                    {sessions.map((s) => (
                      <li key={s.session_id}>
                        <Link
                          to={`/sessions/${s.session_id}`}
                          className="block hover:bg-surface-2 rounded-md px-ds-2 py-ds-1 -mx-ds-2 transition-colors duration-fast ease-ds"
                        >
                          <div className="text-t-small text-text-2">
                            {s.summary || (
                              <span className="italic text-text-4">
                                (요약 없음)
                              </span>
                            )}
                          </div>
                          <div className="text-t-meta text-text-3 mt-0.5 flex items-center gap-ds-2">
                            <span>{s.turn_count} turns</span>
                            <span className="font-mono opacity-70 tabular-nums">
                              {s.session_id.slice(0, 8)}
                            </span>
                          </div>
                        </Link>
                      </li>
                    ))}
                  </ul>
                </article>
              ))}
            </section>
          </div>
        )}
      </div>
    </div>
  );
}
