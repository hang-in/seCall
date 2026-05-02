import { ChevronLeft, ChevronRight, Calendar } from "lucide-react";
import { addDays, format, parseISO, subDays } from "date-fns";
import { Button } from "@/components/ui/button";

interface Props {
  /** 현재 선택된 날짜 (YYYY-MM-DD) */
  value: string;
  /** 새 날짜 선택 콜백 (YYYY-MM-DD) */
  onChange: (next: string) => void;
}

/**
 * 일일 일기용 날짜 네비게이터.
 * - 이전/다음 일자 버튼
 * - native `<input type="date">`로 직접 선택 (Phase 1에서 shadcn DatePicker로 교체 예정)
 * - "오늘" 버튼으로 빠른 이동
 */
export function DateNavigator({ value, onChange }: Props) {
  const today = format(new Date(), "yyyy-MM-dd");
  const isToday = value === today;

  const shift = (delta: number) => {
    const d = parseISO(value);
    onChange(format(delta > 0 ? addDays(d, delta) : subDays(d, -delta), "yyyy-MM-dd"));
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Button
          size="icon"
          variant="ghost"
          aria-label="이전 날짜"
          onClick={() => shift(-1)}
        >
          <ChevronLeft className="size-4" />
        </Button>
        <input
          type="date"
          value={value}
          onChange={(e) => {
            if (e.target.value) onChange(e.target.value);
          }}
          className="bg-transparent border border-border rounded px-2 py-1 text-sm flex-1 tabular-nums"
        />
        <Button
          size="icon"
          variant="ghost"
          aria-label="다음 날짜"
          onClick={() => shift(1)}
        >
          <ChevronRight className="size-4" />
        </Button>
      </div>
      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => onChange(today)}
        disabled={isToday}
      >
        <Calendar className="size-3 mr-2" /> 오늘
      </Button>
    </div>
  );
}
