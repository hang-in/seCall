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
      <aside className="border-r border-border p-4 space-y-4 overflow-auto">
        <DateNavigator value={current} onChange={go} />
        <div className="text-xs text-muted-foreground space-y-1">
          {data && (
            <>
              <div>
                전체 세션: <span className="text-foreground">{data.total_sessions}</span>
              </div>
              <div>
                의미있는 세션:{" "}
                <span className="text-foreground">{data.filtered_sessions}</span>
              </div>
              <div>
                프로젝트:{" "}
                <span className="text-foreground">
                  {Object.keys(data.projects).length}
                </span>
              </div>
            </>
          )}
        </div>
      </aside>

      <div className="overflow-auto p-6 max-w-5xl">
        {isLoading && (
          <div className="flex items-center text-muted-foreground text-sm">
            <Loader2 className="size-4 animate-spin mr-2" /> 불러오는 중…
          </div>
        )}
        {error && (
          <div className="text-rose-400 text-sm">
            {error instanceof Error ? error.message : String(error)}
          </div>
        )}
        {data && !isLoading && (
          <div className="space-y-6">
            <header>
              <h1 className="text-2xl font-semibold tabular-nums">{data.date}</h1>
              <div className="text-sm text-muted-foreground mt-1">
                {data.filtered_sessions === 0
                  ? "이 날의 의미있는 세션이 없습니다"
                  : `${data.filtered_sessions}개 세션 / ${Object.keys(data.projects).length}개 프로젝트`}
              </div>
            </header>

            {data.topics.length > 0 && (
              <section>
                <h2 className="text-xs uppercase tracking-wide text-muted-foreground mb-2">
                  Topics
                </h2>
                <div className="flex flex-wrap gap-1.5">
                  {data.topics.map((t) => (
                    <span
                      key={t}
                      className={`text-xs px-2 py-0.5 rounded ring-1 ${tagColor(t)}`}
                    >
                      {t}
                    </span>
                  ))}
                </div>
              </section>
            )}

            <section className="space-y-4">
              {Object.entries(data.projects).map(([project, sessions]) => (
                <article
                  key={project}
                  className="border border-border rounded-md p-4 bg-card/50"
                >
                  <h3 className="font-semibold mb-3 flex items-baseline gap-2">
                    <span>{project}</span>
                    <span className="text-xs text-muted-foreground font-normal">
                      ({sessions.length})
                    </span>
                  </h3>
                  <ul className="space-y-2">
                    {sessions.map((s) => (
                      <li key={s.session_id}>
                        <Link
                          to={`/sessions/${s.session_id}`}
                          className="block hover:bg-accent rounded px-2 py-1.5 -mx-2 transition-colors"
                        >
                          <div className="text-sm">
                            {s.summary || (
                              <span className="italic text-muted-foreground">
                                (요약 없음)
                              </span>
                            )}
                          </div>
                          <div className="text-[11px] text-muted-foreground mt-0.5 flex items-center gap-2">
                            <span>{s.turn_count} turns</span>
                            <span className="font-mono opacity-60">
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
