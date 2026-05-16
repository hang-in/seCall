import {
  type KeyboardEvent,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { ChevronDown, Loader2, RefreshCw } from "lucide-react";
import { Input } from "@/components/ui/input";
import { useModels, useModelsRefresh } from "@/hooks/useModels";

/**
 * P65 — backend 별 모델 입력 컴포넌트.
 *
 * - `useModels(backend)` 로 `/api/models` 호출 결과를 dropdown 옵션으로 노출.
 * - dynamic 실패 시 서버가 fallback list 를 돌려주므로 빈 옵션 상황은 거의 없음.
 * - 사용자는 자유 입력 가능 (모델 ID 가 비공개/실험적일 수 있어 강제 선택 X).
 *
 * P66 follow-up: native `<input list>` + `<datalist>` 는 시각 cue (chevron)
 * 없고 click 으로도 안 열려 사용자가 dropdown 존재 자체를 모르는 UX 문제.
 * 자체 dropdown panel + chevron + 키보드 nav 로 재구현.
 */
export function ModelInput({
  backend,
  value,
  onChange,
  disabled,
  placeholder,
  ariaLabel,
}: {
  backend: string | null | undefined;
  value: string;
  onChange: (next: string) => void;
  disabled?: boolean;
  placeholder?: string;
  ariaLabel?: string;
}) {
  const id = useId();
  const listboxId = `models-listbox-${id}`;
  const enabled = !!backend && backend !== "disabled";
  const { data, isLoading, isError, refetch, isFetching } = useModels(backend, {
    enabled,
  });
  const refresh = useModelsRefresh();

  const models = useMemo(() => data?.models ?? [], [data]);
  const source = data?.source;

  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // 사용자 입력으로 필터링된 옵션. 비면 전체.
  const filtered = useMemo(() => {
    const q = value.trim().toLowerCase();
    if (!q) return models;
    return models.filter((m) => m.toLowerCase().includes(q));
  }, [models, value]);

  // 외부 click 또는 Escape 로 닫기.
  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      if (!rootRef.current) return;
      if (!rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDocDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // highlight 가 filtered 범위 벗어나면 reset.
  useEffect(() => {
    if (highlight >= filtered.length) setHighlight(0);
  }, [filtered.length, highlight]);

  const commit = (next: string) => {
    onChange(next);
    setOpen(false);
    inputRef.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setHighlight((h) => Math.min(h + 1, Math.max(filtered.length - 1, 0)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setOpen(true);
      setHighlight((h) => Math.max(h - 1, 0));
    } else if (e.key === "Enter") {
      if (open && filtered[highlight]) {
        e.preventDefault();
        commit(filtered[highlight]);
      }
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  const showDropdown = enabled && open && filtered.length > 0;

  return (
    <div className="space-y-1" ref={rootRef}>
      <div className="flex items-center gap-2 relative">
        <div className="relative flex-1">
          <Input
            ref={inputRef}
            aria-label={ariaLabel}
            aria-autocomplete="list"
            aria-expanded={showDropdown}
            aria-controls={showDropdown ? listboxId : undefined}
            role="combobox"
            value={value}
            onChange={(e) => {
              onChange(e.target.value);
              setOpen(true);
            }}
            onFocus={() => enabled && models.length > 0 && setOpen(true)}
            onClick={() => enabled && models.length > 0 && setOpen(true)}
            onKeyDown={onKeyDown}
            disabled={disabled}
            placeholder={placeholder}
            autoComplete="off"
            className="pr-8"
          />
          {enabled && (
            <button
              type="button"
              tabIndex={-1}
              aria-label="모델 목록 열기"
              onClick={() => {
                setOpen((v) => !v);
                inputRef.current?.focus();
              }}
              className="absolute inset-y-0 right-0 flex items-center px-2 text-text-3 hover:text-text"
              disabled={disabled || models.length === 0}
            >
              <ChevronDown
                className={`size-4 transition-transform duration-fast ${
                  showDropdown ? "rotate-180" : ""
                }`}
              />
            </button>
          )}
          {showDropdown && (
            <ul
              id={listboxId}
              role="listbox"
              className="absolute left-0 right-0 top-full mt-1 z-20 max-h-60 overflow-auto rounded-md border border-hairline bg-[var(--surface)] shadow-lg py-1"
            >
              {filtered.map((m, i) => (
                <li
                  key={m}
                  role="option"
                  aria-selected={i === highlight}
                  onMouseDown={(e) => {
                    // mousedown 으로 처리 (click 보다 먼저) → input blur 전에 commit.
                    e.preventDefault();
                    commit(m);
                  }}
                  onMouseEnter={() => setHighlight(i)}
                  className={`px-3 py-1.5 text-t-small cursor-pointer ${
                    i === highlight
                      ? "bg-surface-2 text-text"
                      : "text-text-2 hover:bg-surface-2"
                  }`}
                >
                  {m}
                </li>
              ))}
            </ul>
          )}
        </div>
        {enabled && (
          <button
            type="button"
            onClick={async () => {
              if (!backend) return;
              await refresh(backend);
              await refetch();
            }}
            disabled={disabled || isFetching}
            className="inline-flex h-9 items-center justify-center rounded-md border border-hairline px-2 text-text-3 hover:text-text disabled:opacity-50"
            aria-label="모델 목록 새로고침"
            title="모델 목록 새로고침"
          >
            {isFetching ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <RefreshCw className="size-3.5" />
            )}
          </button>
        )}
      </div>
      {enabled && isLoading && (
        <p className="text-t-meta text-text-3">모델 목록 로드 중…</p>
      )}
      {enabled && !isLoading && isError && (
        <p className="text-t-meta text-status-danger">
          모델 목록 조회 실패 — 직접 입력하세요.
        </p>
      )}
      {enabled && source === "fallback" && (
        <p className="text-t-meta text-text-3">
          fallback 목록 표시 중 (backend 응답 없음 — 직접 입력 가능)
        </p>
      )}
    </div>
  );
}
