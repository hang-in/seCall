import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import type { SearchMode } from "@/lib/types";

interface Props {
  /** 디바운스 적용된 최종 값 (부모가 보유). */
  value: string;
  /** 디바운스 후 호출됨. */
  onChange: (next: string) => void;
  mode?: SearchMode;
  onModeChange?: (next: SearchMode) => void;
  placeholder?: string;
  debounceMs?: number;
}

/**
 * 검색 입력 + 모드 토글 (keyword/semantic).
 * - 디바운스 300ms (lodash 없이 setTimeout)
 * - X 버튼으로 즉시 클리어
 * - 시맨틱 호출은 향후 별도 훅에서 처리. 본 컴포넌트는 mode 상태만 관리.
 */
export function SearchBar({
  value,
  onChange,
  mode = "keyword",
  onModeChange,
  placeholder = "세션 검색...",
  debounceMs = 300,
}: Props) {
  const [local, setLocal] = useState(value);
  const timerRef = useRef<number | null>(null);

  // 외부에서 value가 바뀌면 (예: 초기화) local도 동기화
  useEffect(() => {
    setLocal(value);
  }, [value]);

  useEffect(() => {
    if (local === value) return;
    if (timerRef.current) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => {
      onChange(local);
    }, debounceMs);
    return () => {
      if (timerRef.current) window.clearTimeout(timerRef.current);
    };
    // local만 트리거. onChange/value는 의도적으로 제외 (refs/외부 업데이트 루프 방지)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local, debounceMs]);

  const clear = () => {
    setLocal("");
    onChange("");
  };

  return (
    <div className="flex items-center gap-2">
      <div className="relative flex-1">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-4 text-muted-foreground pointer-events-none" />
        <Input
          value={local}
          onChange={(e) => setLocal(e.target.value)}
          placeholder={placeholder}
          className="pl-8 pr-8 h-9"
          data-hotkey="search"
        />
        {local && (
          <button
            type="button"
            onClick={clear}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            aria-label="검색어 지우기"
          >
            <X className="size-4" />
          </button>
        )}
      </div>
      {onModeChange && (
        <div className="flex rounded-md border border-border overflow-hidden">
          <Button
            type="button"
            variant={mode === "keyword" ? "secondary" : "ghost"}
            size="sm"
            className="h-9 rounded-none px-2 text-xs"
            onClick={() => onModeChange("keyword")}
          >
            키워드
          </Button>
          <Button
            type="button"
            variant={mode === "semantic" ? "secondary" : "ghost"}
            size="sm"
            className="h-9 rounded-none px-2 text-xs"
            onClick={() => onModeChange("semantic")}
          >
            시맨틱
          </Button>
        </div>
      )}
    </div>
  );
}
