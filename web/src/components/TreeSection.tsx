import { useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";

/**
 * 접히는 그룹 카드 — prototype layout.jsx 의 TreeSection 패턴 (Stage 8).
 *
 * 헤더: chevron + 라벨(mono) + (옵션) 활성 배지 + 우측 카운트
 * 본문: 펼친 상태에서만 children 렌더 (퍼포먼스 + 시각)
 *
 * 사용처: Daily 라우트의 날짜 그룹, Sessions 의 좌측 하단 필터 등.
 */
interface Props {
  title: string;
  /** 우측 mono 카운트 (string 또는 number, "5월 1주차" 같이 임의 라벨도 가능) */
  count?: ReactNode;
  /** 라벨 옆 작은 brand 배지 (예: 활성 필터 개수) */
  badge?: ReactNode;
  defaultOpen?: boolean;
  variant?: "default" | "filter";
  children: ReactNode;
}

export function TreeSection({
  title,
  count,
  badge,
  defaultOpen = true,
  variant = "default",
  children,
}: Props) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="border-b border-hairline last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="w-full flex items-center gap-ds-2 px-ds-3 py-ds-2 text-text-3 hover:text-text hover:bg-surface-2 transition-colors duration-fast ease-ds"
      >
        <ChevronRight
          className={`size-3 transition-transform duration-fast ease-ds ${
            open ? "rotate-90" : ""
          }`}
          aria-hidden
        />
        <span className="font-mono text-t-mono text-text-2">{title}</span>
        {badge != null && (
          <span className="font-mono text-t-caption px-1 py-0.5 rounded-sm bg-brand-soft text-brand">
            {badge}
          </span>
        )}
        <span className="flex-1" />
        {count != null && (
          <span className="font-mono text-t-mono text-text-3 tabular-nums">
            {count}
          </span>
        )}
      </button>
      {open && (
        <div
          className={
            variant === "filter"
              ? "px-ds-3 pb-ds-3 pt-ds-2"
              : "py-ds-1"
          }
        >
          {children}
        </div>
      )}
    </div>
  );
}
