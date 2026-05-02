import { useHotkeys } from "react-hotkeys-hook";

interface ListItem {
  id: string;
}

interface Options {
  /** Enter 키 입력 시 실행. 미지정이면 onSelect를 한 번 더 호출. */
  onEnter?: (id: string) => void;
  /** 활성 여부. 외부에서 false 주면 단축키 비활성. */
  enabled?: boolean;
}

/**
 * 리스트 컨텍스트 단축키.
 *
 * - `j`: 다음 항목으로 selection 이동
 * - `k`: 이전 항목으로 selection 이동
 * - `Enter`: 현재 선택 항목 확정 (onEnter 또는 onSelect 재호출)
 * - `[`: 이전 항목으로 navigate (selectedId 기반)
 * - `]`: 다음 항목으로 navigate (selectedId 기반)
 *
 * 단축키가 의미를 가지려면 호출하는 컴포넌트가 selectedId를 보유하고 있어야 한다.
 * 입력 필드 안에서는 react-hotkeys-hook 기본값으로 비활성.
 */
export function useListHotkeys(
  items: ListItem[],
  selectedId: string | undefined,
  onSelect: (id: string) => void,
  options: Options = {},
) {
  const { onEnter, enabled = true } = options;

  useHotkeys(
    "j",
    () => {
      if (!items.length) return;
      const idx = items.findIndex((i) => i.id === selectedId);
      const nextIdx = idx < 0 ? 0 : Math.min(idx + 1, items.length - 1);
      const next = items[nextIdx];
      if (next) onSelect(next.id);
    },
    { enabled },
    [items, selectedId, onSelect, enabled],
  );

  useHotkeys(
    "k",
    () => {
      if (!items.length) return;
      const idx = items.findIndex((i) => i.id === selectedId);
      const prevIdx = idx < 0 ? 0 : Math.max(idx - 1, 0);
      const prev = items[prevIdx];
      if (prev) onSelect(prev.id);
    },
    { enabled },
    [items, selectedId, onSelect, enabled],
  );

  useHotkeys(
    "enter",
    () => {
      if (!selectedId) return;
      if (onEnter) onEnter(selectedId);
      else onSelect(selectedId);
    },
    { enabled },
    [selectedId, onSelect, onEnter, enabled],
  );

  useHotkeys(
    "[",
    () => {
      if (!items.length || !selectedId) return;
      const idx = items.findIndex((i) => i.id === selectedId);
      if (idx <= 0) return;
      const prev = items[idx - 1];
      if (prev) onSelect(prev.id);
    },
    { enabled },
    [items, selectedId, onSelect, enabled],
  );

  useHotkeys(
    "]",
    () => {
      if (!items.length || !selectedId) return;
      const idx = items.findIndex((i) => i.id === selectedId);
      if (idx < 0 || idx >= items.length - 1) return;
      const next = items[idx + 1];
      if (next) onSelect(next.id);
    },
    { enabled },
    [items, selectedId, onSelect, enabled],
  );
}
