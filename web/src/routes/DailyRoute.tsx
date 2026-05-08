import { useMemo } from "react";
import { useNavigate, useParams, Link } from "react-router";
import { addDays, format, parseISO, startOfWeek, subDays } from "date-fns";
import { ChevronLeft, ChevronRight, Loader2 } from "lucide-react";
import { useDaily } from "@/hooks/useDaily";
import { TreeSection } from "@/components/TreeSection";
import { tagColor } from "@/lib/tagColor";

/**
 * 일일 일기 라우트 (Stage 8).
 *
 * 좌측 list-w: TreeSection 두 그룹
 *   - "이번 주" — 월요일 ~ 오늘
 *   - "지난 주" — 그 전 7일
 *   각 항목: 요일(mono) + 날짜(mono) + 클릭 시 라우트 이동
 * 우측: pane--prose 형태 — crumbs + 본문 (topics + project 별 세션 + 자동 일기)
 *
 * 세션 카운트 / 턴 수는 backend 의 `/api/daily/index` 같은 endpoint 가 없어 좌측 트리에서는 생략.
 * 클릭 시 fetch 된 data 의 카운트는 우측 헤더에서 표시.
 */
export default function DailyRoute() {
  const { date } = useParams<{ date?: string }>();
  const navigate = useNavigate();
  const today = format(new Date(), "yyyy-MM-dd");
  const current = date ?? today;
  const { data, isLoading, error } = useDaily(current);

  // 이번 주 (월요일 ~ 오늘) + 지난 주 (그 전 7일) 의 날짜 list
  const { thisWeek, lastWeek } = useMemo(() => {
    const weekStart = startOfWeek(new Date(), { weekStartsOn: 1 });
    const tw: string[] = [];
    for (let d = parseISO(today); d >= weekStart; d = subDays(d, 1)) {
      tw.push(format(d, "yyyy-MM-dd"));
    }
    const lw: string[] = [];
    for (let i = 1; i <= 7; i++) {
      lw.push(format(subDays(weekStart, i), "yyyy-MM-dd"));
    }
    return { thisWeek: tw, lastWeek: lw };
  }, [today]);

  const go = (d: string) => navigate(`/daily/${d}`);

  return (
    <div className="grid grid-cols-[var(--list-w)_1fr] h-full">
      {/* 좌측 트리 */}
      <aside className="border-r border-hairline bg-[var(--surface)] overflow-auto flex flex-col">
        <div className="flex items-center gap-ds-1 p-ds-3 border-b border-hairline">
          <button
            type="button"
            onClick={() => go(format(subDays(parseISO(current), 1), "yyyy-MM-dd"))}
            aria-label="이전 날짜"
            className="size-7 inline-flex items-center justify-center rounded-md text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
          >
            <ChevronLeft className="size-4" />
          </button>
          <input
            type="date"
            value={current}
            onChange={(e) => e.target.value && go(e.target.value)}
            className="flex-1 bg-transparent border border-border-soft rounded-md px-ds-2 py-1 text-t-small font-mono tabular-nums text-text-2 focus:outline-none focus:ring-2 focus:ring-brand-soft focus:border-brand"
          />
          <button
            type="button"
            onClick={() => go(format(addDays(parseISO(current), 1), "yyyy-MM-dd"))}
            aria-label="다음 날짜"
            className="size-7 inline-flex items-center justify-center rounded-md text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
          >
            <ChevronRight className="size-4" />
          </button>
        </div>

        <div className="flex-1 overflow-auto">
          <TreeSection title="이번 주" count={`${thisWeek.length}일`}>
            <DateList dates={thisWeek} current={current} onClick={go} />
          </TreeSection>
          <TreeSection title="지난 주" count={`${lastWeek.length}일`} defaultOpen={false}>
            <DateList dates={lastWeek} current={current} onClick={go} />
          </TreeSection>
        </div>
      </aside>

      {/* 우측 pane */}
      <div className="overflow-auto bg-[var(--bg)]">
        <div className="p-ds-6 max-w-[var(--read-w)] mx-auto">
          {isLoading && (
            <div className="flex items-center text-t-small text-text-3">
              <Loader2 className="size-4 animate-spin mr-ds-2" /> 불러오는 중…
            </div>
          )}
          {error && (
            <div className="text-t-small text-status-danger">
              {error instanceof Error ? error.message : String(error)}
            </div>
          )}
          {data && !isLoading && (
            <article className="space-y-ds-6">
              {/* head crumbs */}
              <header className="flex items-center gap-ds-2 text-t-meta text-text-3 border-b border-hairline pb-ds-3">
                <span className="eyebrow">Daily</span>
                <span aria-hidden className="text-text-4">/</span>
                <span className="font-mono text-t-mono text-text-2 tabular-nums">
                  {data.date}
                </span>
                <span className="flex-1" />
                <span className="font-mono text-t-mono">
                  {data.filtered_sessions} / {data.total_sessions} 세션 ·{" "}
                  {Object.keys(data.projects).length} 프로젝트
                </span>
              </header>

              {/* topics */}
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

              {/* projects */}
              {Object.keys(data.projects).length === 0 && (
                <div className="text-t-small text-text-3 italic">
                  이 날의 의미있는 세션이 없습니다.
                </div>
              )}
              {Object.entries(data.projects).map(([project, sessions]) => (
                <section key={project} className="space-y-ds-3">
                  <h2 className="text-t-h1 font-medium tracking-tight flex items-baseline gap-ds-2">
                    <span className="font-mono text-text">{project}</span>
                    <span className="text-t-meta text-text-3 font-normal font-sans tabular-nums">
                      ({sessions.length})
                    </span>
                  </h2>
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
                            <span className="font-mono tabular-nums">
                              {s.turn_count} turns
                            </span>
                            <span aria-hidden className="text-text-4">·</span>
                            <span className="font-mono opacity-70 tabular-nums">
                              {s.session_id.slice(0, 8)}
                            </span>
                          </div>
                        </Link>
                      </li>
                    ))}
                  </ul>
                </section>
              ))}
            </article>
          )}
        </div>
      </div>
    </div>
  );
}

interface DateListProps {
  dates: string[];
  current: string;
  onClick: (d: string) => void;
}

function DateList({ dates, current, onClick }: DateListProps) {
  return (
    <ul>
      {dates.map((d) => {
        const dt = parseISO(d);
        const wd = format(dt, "EEE");
        const selected = d === current;
        return (
          <li key={d}>
            <button
              type="button"
              onClick={() => onClick(d)}
              className={[
                "w-full flex items-center gap-ds-3 px-ds-4 py-ds-1 text-t-small transition-colors duration-fast ease-ds border-l-2",
                selected
                  ? "border-l-brand bg-surface-2 text-text font-medium"
                  : "border-l-transparent text-text-3 hover:text-text hover:bg-surface-2",
              ].join(" ")}
            >
              <span className="font-mono w-7 text-text-3">{wd}</span>
              <span className="font-mono tabular-nums">{d}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
