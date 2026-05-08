import { useEffect, useRef, useState } from "react";
import { Search, X } from "lucide-react";
import { useUi, type GlobalSearchMode } from "@/lib/store";

/**
 * TopNav 중앙 슬롯 — 전역 검색 입력 + mode 토글.
 *
 * - 검색 state 는 `useUi` store 에 lift (route 간 유지).
 * - 디바운스 200ms (입력 후 멈추면 store 갱신).
 * - mode 후보는 props 로 받음 — sessions: ["keyword","semantic"], wiki: ["keyword","semantic","hybrid"].
 * - prototype layout.jsx 의 HeaderSearch (hsb) 패턴.
 */
interface Props {
  modes: readonly GlobalSearchMode[];
  placeholder?: string;
}

export function HeaderSearch({ modes, placeholder }: Props) {
  const query = useUi((s) => s.query);
  const setQuery = useUi((s) => s.setQuery);
  const mode = useUi((s) => s.searchMode);
  const setMode = useUi((s) => s.setSearchMode);

  const [local, setLocal] = useState(query);
  const [focus, setFocus] = useState(false);
  const timerRef = useRef<number | null>(null);

  // 외부 변경(다른 라우트 진입 등) → local 동기화
  useEffect(() => {
    setLocal(query);
  }, [query]);

  // 라우트 변경 등으로 mode 가 현재 modes 에 없으면 첫 모드로 폴백
  useEffect(() => {
    if (!modes.includes(mode)) setMode(modes[0]);
  }, [modes, mode, setMode]);

  // 디바운스 200ms
  useEffect(() => {
    if (local === query) return;
    if (timerRef.current) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => setQuery(local), 200);
    return () => {
      if (timerRef.current) window.clearTimeout(timerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local]);

  const ph =
    placeholder ??
    (mode === "semantic" ? "의미로 검색…" : mode === "hybrid" ? "전체 검색…" : "검색…");

  return (
    <div className="flex items-center gap-ds-2 w-full max-w-[480px]">
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
              onClick={() => {
                setLocal("");
                setQuery("");
              }}
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

      {modes.length > 1 && (
        <div className="flex rounded-md border border-border-soft overflow-hidden bg-[var(--surface)] shrink-0">
          {modes.map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setMode(m)}
              title={
                m === "keyword"
                  ? "키워드 (BM25)"
                  : m === "semantic"
                    ? "시맨틱 (벡터)"
                    : "하이브리드 (RRF)"
              }
              className={[
                "px-ds-2 h-8 text-t-meta transition-colors duration-fast ease-ds",
                mode === m
                  ? "bg-surface-2 text-text font-medium"
                  : "text-text-3 hover:text-text hover:bg-surface-2",
              ].join(" ")}
            >
              {m[0].toUpperCase() + m.slice(1)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
