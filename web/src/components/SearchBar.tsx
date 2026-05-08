import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import type { SearchMode } from "@/lib/types";

/**
 * 검색 입력 + keyword/semantic 모드 토글 — Calm/Editorial 톤 (Stage 2c).
 *
 * - 디바운스 300ms (외부 onChange 발화는 입력 멈춘 뒤).
 * - clear (X) 버튼 / 우측에 `/` 단축키 hint kbd.
 * - segmented 모드 토글 (양쪽 둥근 inline border).
 * - 모든 시각은 design tokens (web/src/lib/design-tokens.md) 사용.
 */
interface Props {
  value: string;
  onChange: (next: string) => void;
  mode?: SearchMode;
  onModeChange?: (next: SearchMode) => void;
  placeholder?: string;
  debounceMs?: number;
}

export function SearchBar({
  value,
  onChange,
  mode = "keyword",
  onModeChange,
  placeholder,
  debounceMs = 300,
}: Props) {
  const [local, setLocal] = useState(value);
  const [focus, setFocus] = useState(false);
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    setLocal(value);
  }, [value]);

  useEffect(() => {
    if (local === value) return;
    if (timerRef.current) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => onChange(local), debounceMs);
    return () => {
      if (timerRef.current) window.clearTimeout(timerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local, debounceMs]);

  const clear = () => {
    setLocal("");
    onChange("");
  };

  const ph =
    placeholder ?? (mode === "semantic" ? "의미로 검색…" : "세션 검색…");

  return (
    <div className="flex items-center gap-ds-2">
      <div
        className={[
          "flex-1 flex items-center h-8 rounded-md border bg-[var(--surface)] transition-colors duration-fast ease-ds",
          focus
            ? "border-brand ring-2 ring-brand-soft"
            : "border-border-soft hover:border-border-strong",
        ].join(" ")}
      >
        <Search className="size-3.5 ml-ds-2 text-text-3 pointer-events-none" />
        <input
          type="text"
          value={local}
          placeholder={ph}
          onChange={(e) => setLocal(e.target.value)}
          onFocus={() => setFocus(true)}
          onBlur={() => setFocus(false)}
          data-hotkey="search"
          className="flex-1 px-ds-2 bg-transparent text-t-body text-text placeholder:text-text-4 outline-none"
        />
        <span className="pr-ds-2 text-text-3">
          {local ? (
            <button
              type="button"
              onClick={clear}
              aria-label="검색어 지우기"
              className="inline-flex items-center justify-center size-4 rounded-sm hover:text-text"
            >
              <X className="size-3" />
            </button>
          ) : (
            <kbd className="kbd">/</kbd>
          )}
        </span>
      </div>

      {onModeChange && (
        <div className="flex rounded-md border border-border-soft overflow-hidden bg-[var(--surface)]">
          {(["keyword", "semantic"] as const).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => onModeChange(m)}
              title={m === "keyword" ? "키워드 (BM25)" : "시맨틱 (벡터)"}
              className={[
                "px-ds-3 h-8 text-t-meta transition-colors duration-fast ease-ds",
                mode === m
                  ? "bg-surface-2 text-text font-medium"
                  : "text-text-3 hover:text-text hover:bg-surface-2",
              ].join(" ")}
            >
              {m === "keyword" ? "키워드" : "시맨틱"}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
