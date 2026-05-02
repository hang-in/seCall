import { useEffect, useRef, useState } from "react";
import { Check, CircleDashed, Loader2 } from "lucide-react";
import { useDebounce } from "@/hooks/useDebounce";
import { useSetNotes } from "@/hooks/useTagMutations";

interface Props {
  sessionId: string;
  initial: string | null | undefined;
}

type SaveState = "idle" | "dirty" | "saving" | "saved" | "error";

/**
 * 세션 노트 편집기 (P34 Task 08).
 *
 * - autosave: 사용자 입력 후 1초 idle → PATCH `/api/sessions/{id}/notes`
 * - 저장 상태: idle / dirty(변경됨) / saving(저장 중) / saved(저장됨) / error
 * - `<details>`로 접혀 마운트되며 `data-hotkey-anchor="notes"`(details) +
 *   `data-hotkey="notes"`(textarea)로 Task 05의 `e` 단축키 연결점을 제공한다.
 * - 빈 문자열은 raw 저장 (사용자 의도 보존; 백엔드는 ""와 null 모두 허용).
 */
export function NoteEditor({ sessionId, initial }: Props) {
  const [text, setText] = useState(initial ?? "");
  const debounced = useDebounce(text, 1000);
  const [state, setState] = useState<SaveState>("idle");
  const mutation = useSetNotes(sessionId);
  const lastSaved = useRef(initial ?? "");

  // 다른 세션으로 이동 시 prop 변화에 동기화
  useEffect(() => {
    setText(initial ?? "");
    lastSaved.current = initial ?? "";
    setState("idle");
  }, [sessionId, initial]);

  // 사용자가 입력하는 즉시 dirty 상태로
  useEffect(() => {
    if (text !== lastSaved.current) setState("dirty");
  }, [text]);

  // debounced 값이 바뀌고 lastSaved와 다르면 저장
  useEffect(() => {
    if (debounced === lastSaved.current) return;
    setState("saving");
    mutation.mutate(debounced || null, {
      onSuccess: () => {
        lastSaved.current = debounced;
        setState("saved");
      },
      onError: () => setState("error"),
    });
    // mutation 인스턴스는 매 렌더 새로 만들어지므로 deps에서 제외 (debounced만 트리거)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debounced]);

  return (
    <details
      className="border-t border-border pt-3 mt-3"
      data-hotkey-anchor="notes"
    >
      <summary className="cursor-pointer text-sm font-medium flex items-center gap-2">
        노트
        <SaveIndicator state={state} />
      </summary>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="이 세션에 대한 메모..."
        rows={4}
        data-hotkey="notes"
        className="mt-2 w-full bg-background border border-border rounded p-2 text-sm font-mono resize-y focus:outline-none focus:ring-1 focus:ring-ring"
      />
    </details>
  );
}

function SaveIndicator({ state }: { state: SaveState }) {
  if (state === "saving")
    return (
      <span className="text-xs text-muted-foreground inline-flex items-center gap-1">
        <Loader2 className="size-3 animate-spin" />
        저장 중
      </span>
    );
  if (state === "saved")
    return (
      <span className="text-xs text-emerald-400 inline-flex items-center gap-1">
        <Check className="size-3" />
        저장됨
      </span>
    );
  if (state === "dirty")
    return (
      <span className="text-xs text-amber-400 inline-flex items-center gap-1">
        <CircleDashed className="size-3" />
        변경됨
      </span>
    );
  if (state === "error")
    return <span className="text-xs text-rose-400">저장 실패</span>;
  return null;
}
