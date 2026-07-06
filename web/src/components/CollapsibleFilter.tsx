import { useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import type { SessionFilterState } from "@/lib/types";

/**
 * 좌측 패널 하단의 접히는 필터 카드 — prototype layout.jsx 의 CollapsibleFilter (TreeSection variant) 패턴.
 *
 * - 헤더: chevron + 라벨(mono) + active 카운트 배지 + 결과 카운트(우측, mono)
 * - 본문: 펼친 상태에서만 children 렌더 — 본 컴포넌트는 SessionFilters 본문을 그대로 받음
 * - defaultOpen=false (접힌 상태)
 */
interface Props {
  filters: SessionFilterState;
  resultCount: number | null;
  children: ReactNode;
  defaultOpen?: boolean;
}

export function CollapsibleFilter({
  filters,
  resultCount,
  children,
  defaultOpen = false,
}: Props) {
  const [open, setOpen] = useState(defaultOpen);

  const activeCount = countActive(filters);

  return (
    <div className="border-t border-hairline bg-[var(--surface)]">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-ds-2 px-ds-3 py-ds-2 text-t-meta text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
        aria-expanded={open}
      >
        <ChevronRight
          className={`size-3 transition-transform duration-fast ease-ds ${
            open ? "rotate-90" : ""
          }`}
          aria-hidden
        />
        <span className="font-mono text-t-mono text-text-2">필터</span>
        {activeCount > 0 && (
          <span className="font-mono text-t-caption px-1 py-0.5 rounded-sm bg-brand-soft text-brand">
            {activeCount}
          </span>
        )}
        <span className="flex-1" />
        {resultCount !== null && (
          <span className="font-mono text-t-mono text-text-3 tabular-nums">
            {resultCount}건
          </span>
        )}
      </button>
      {open && (
        <div className="px-ds-3 pb-ds-3 border-t border-hairline pt-ds-3">{children}</div>
      )}
    </div>
  );
}

function countActive(f: SessionFilterState): number {
  let n = 0;
  if (f.project) n++;
  if (f.agent) n++;
  if (f.date_from) n++;
  if (f.date_to) n++;
  if (f.tags && f.tags.length > 0) n += f.tags.length;
  if (f.favorite) n++;
  if (f.tag) n++;
  if (f.include_automated) n++;
  return n;
}
