import { useMemo, useState } from "react";
import {
  addMonths,
  eachDayOfInterval,
  endOfMonth,
  format,
  getDay,
  startOfMonth,
} from "date-fns";
import { Calendar as CalendarIcon, ChevronLeft, ChevronRight } from "lucide-react";
import { useSessionCalendar, type CalendarFilters } from "@/hooks/useSessions";

/**
 * Phase 3 — 좌측 패널 상단 접이식 미니 캘린더.
 *
 * - 월 단위 그리드 + 날짜별 세션 수 배지(0이면 생략).
 * - 날짜 클릭 → 해당 로컬 날짜로 date 필터(date_from=date_to) 적용. 재클릭 시 해제.
 * - 이전/다음 달 이동. 표시 중인 월 범위만 count API 호출(월 이동 시 자동 refetch).
 * - 기본 접힘(defaultOpen=false)이라 열기 전에는 API 호출/레이아웃 영향 없음.
 * - index.css 디자인 토큰만 사용 → 다크/라이트 자동 대응.
 */
const WEEKDAYS = ["일", "월", "화", "수", "목", "금", "토"];

interface Props {
  /** 현재 date 필터가 단일 날짜(date_from===date_to)면 그 값. 셀 하이라이트용. */
  selectedDate?: string;
  /** 날짜 선택/해제 콜백. undefined 면 필터 해제 의도. */
  onSelectDate: (date: string | undefined) => void;
  /** 리스트와 동일한 활성 필터(project/agent/tag/favorite/include_automated/q).
   *  배지 카운트를 필터된 리스트와 일치시킨다. date_from/date_to 는 제외. */
  filters?: CalendarFilters;
  defaultOpen?: boolean;
}

export function MiniCalendar({
  selectedDate,
  onSelectDate,
  filters,
  defaultOpen = false,
}: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const [cursor, setCursor] = useState(() => startOfMonth(new Date()));

  const monthEnd = useMemo(() => endOfMonth(cursor), [cursor]);
  const from = format(cursor, "yyyy-MM-dd");
  const to = format(monthEnd, "yyyy-MM-dd");
  const today = format(new Date(), "yyyy-MM-dd");
  // tz 오프셋(분, 로컬-UTC)을 '표시 중인 월 중순' 기준으로 계산한다. 모듈 로드 시
  // 1회 고정하면 DST 지역에서 다른 DST 구간의 월에 잘못된 오프셋을 보내므로,
  // cursor 가 바뀔 때마다 그 월에 맞는 오프셋을 다시 구한다. getTimezoneOffset 은
  // UTC-로컬이라 부호 반전 (useDaily 와 동일).
  const tzOffset = useMemo(
    () =>
      -new Date(cursor.getFullYear(), cursor.getMonth(), 15).getTimezoneOffset(),
    [cursor],
  );

  const { data } = useSessionCalendar(from, to, tzOffset, filters ?? {}, {
    enabled: open,
  });

  const counts = useMemo(() => {
    const m = new Map<string, number>();
    (data ?? []).forEach((d) => m.set(d.date, d.count));
    return m;
  }, [data]);

  const days = useMemo(
    () => eachDayOfInterval({ start: cursor, end: monthEnd }),
    [cursor, monthEnd],
  );
  const leadingBlanks = getDay(cursor); // 0=일 ~ 6=토

  return (
    <div className="border-b border-hairline bg-surface">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="flex w-full items-center gap-ds-2 px-ds-3 py-ds-2 text-t-meta text-text-3 transition-colors duration-fast ease-ds hover:bg-surface-2 hover:text-text"
      >
        <CalendarIcon className="size-3.5" aria-hidden />
        <span className="font-mono text-t-mono text-text-2">달력</span>
        <span className="flex-1" />
        <ChevronRight
          className={`size-3 transition-transform duration-fast ease-ds ${open ? "rotate-90" : ""}`}
          aria-hidden
        />
      </button>

      {open && (
        <div className="border-t border-hairline px-ds-3 pb-ds-3 pt-ds-2">
          {/* 월 네비게이션 */}
          <div className="mb-ds-2 flex items-center justify-between">
            <button
              type="button"
              onClick={() => setCursor((c) => addMonths(c, -1))}
              aria-label="이전 달"
              className="flex size-6 items-center justify-center rounded-sm text-text-3 hover:bg-surface-2 hover:text-text"
            >
              <ChevronLeft className="size-4" />
            </button>
            <span className="font-mono text-t-small tabular-nums text-text-2">
              {format(cursor, "yyyy.MM")}
            </span>
            <button
              type="button"
              onClick={() => setCursor((c) => addMonths(c, 1))}
              aria-label="다음 달"
              className="flex size-6 items-center justify-center rounded-sm text-text-3 hover:bg-surface-2 hover:text-text"
            >
              <ChevronRight className="size-4" />
            </button>
          </div>

          {/* 요일 헤더 */}
          <div className="mb-0.5 grid grid-cols-7 gap-0.5">
            {WEEKDAYS.map((w) => (
              <div
                key={w}
                className="text-center text-t-caption text-text-4"
                aria-hidden
              >
                {w}
              </div>
            ))}
          </div>

          {/* 날짜 그리드 */}
          <div className="grid grid-cols-7 gap-0.5">
            {Array.from({ length: leadingBlanks }).map((_, i) => (
              <div key={`blank-${i}`} aria-hidden />
            ))}
            {days.map((day) => {
              const ds = format(day, "yyyy-MM-dd");
              const count = counts.get(ds) ?? 0;
              const isSelected = selectedDate === ds;
              const isToday = ds === today;
              const stateCls = isSelected
                ? "bg-brand-soft text-brand ring-1 ring-brand font-medium"
                : isToday
                  ? "text-brand font-medium hover:bg-surface-2"
                  : count > 0
                    ? "text-text hover:bg-surface-2"
                    : "text-text-4 hover:bg-surface-2";
              return (
                <button
                  key={ds}
                  type="button"
                  onClick={() => onSelectDate(isSelected ? undefined : ds)}
                  aria-pressed={isSelected}
                  aria-label={`${ds} 세션 ${count}개`}
                  className={`relative flex aspect-square flex-col items-center justify-center rounded-sm text-t-caption tabular-nums transition-colors duration-fast ease-ds ${stateCls}`}
                >
                  <span className="leading-none">{format(day, "d")}</span>
                  {count > 0 && (
                    <span className="mt-0.5 text-[10px] leading-none tabular-nums text-brand">
                      {count}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
