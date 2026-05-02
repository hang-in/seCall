import { useEffect, useState } from "react";

/**
 * Generic debounce 훅 (P34 Task 08).
 *
 * `value`가 변경된 후 `delay`ms 동안 추가 변경이 없을 때만 debounced 값이 업데이트된다.
 * NoteEditor에서 사용자 입력을 1초 간격으로 묶어 PATCH 요청 빈도를 제어하는 데 사용.
 */
export function useDebounce<T>(value: T, delay = 1000): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(t);
  }, [value, delay]);
  return debounced;
}
