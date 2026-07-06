import { ArrowDown, ArrowUp } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { SessionSort, SortOrder } from "@/lib/types";

/**
 * Phase 1 — 좌측 리스트 상단 정렬 컨트롤.
 *
 * 정렬 기준(날짜/턴 수/프로젝트/에이전트) shadcn Select + asc/desc 토글 버튼.
 * keyword 모드에서만 렌더된다(semantic 은 score 정렬이라 SessionsRoute 에서 숨김).
 */
const SORT_LABELS: Record<SessionSort, string> = {
  date: "날짜",
  turns: "턴 수",
  project: "프로젝트",
  agent: "에이전트",
};

const SORT_KEYS = Object.keys(SORT_LABELS) as SessionSort[];

interface Props {
  sort: SessionSort;
  order: SortOrder;
  onSortChange: (next: SessionSort) => void;
  onOrderChange: (next: SortOrder) => void;
}

export function SessionSortControl({
  sort,
  order,
  onSortChange,
  onOrderChange,
}: Props) {
  return (
    <div className="flex items-center gap-ds-2 border-b border-hairline bg-surface px-ds-3 py-ds-2">
      <span className="shrink-0 text-t-meta text-text-3">정렬</span>
      <Select value={sort} onValueChange={(v) => onSortChange(v as SessionSort)}>
        <SelectTrigger className="h-8 flex-1 text-xs" aria-label="정렬 기준">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SORT_KEYS.map((k) => (
            <SelectItem key={k} value={k}>
              {SORT_LABELS[k]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="h-8 w-8 shrink-0 p-0"
        aria-label={
          order === "asc"
            ? "오름차순 — 클릭 시 내림차순"
            : "내림차순 — 클릭 시 오름차순"
        }
        title={order === "asc" ? "오름차순" : "내림차순"}
        onClick={() => onOrderChange(order === "asc" ? "desc" : "asc")}
      >
        {order === "asc" ? (
          <ArrowUp className="size-4" />
        ) : (
          <ArrowDown className="size-4" />
        )}
      </Button>
    </div>
  );
}
