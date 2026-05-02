import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { tagColor } from "@/lib/tagColor";
import { useSetTags } from "@/hooks/useTagMutations";
import { useAllTags } from "@/lib/allTags";

interface Props {
  sessionId: string;
  initial: string[];
}

/**
 * 세션 태그 편집기.
 * - 칩 형태로 현재 태그 표시 (각 칩 우측 X로 삭제)
 * - 입력 필드에서 Enter 또는 쉼표로 추가
 * - 입력값에 prefix 매칭되는 자동완성 노출 (allTags 기반)
 * - 모든 변경은 `setTags` mutation을 통해 서버 정규화된 결과로 갱신된다.
 */
export function TagEditor({ sessionId, initial }: Props) {
  const [tags, setTags] = useState<string[]>(initial);
  const [draft, setDraft] = useState("");
  const mutation = useSetTags(sessionId);
  const allTags = useAllTags();

  // 다른 세션으로 이동했을 때 / 서버 invalidate 후 props가 바뀐 경우 동기화
  useEffect(() => {
    setTags(initial);
  }, [initial, sessionId]);

  const commit = async (next: string[]) => {
    const prev = tags;
    setTags(next); // 낙관적
    try {
      const res = await mutation.mutateAsync(next);
      setTags(res.tags); // 서버 정규화 결과로 보정
    } catch {
      setTags(prev); // 롤백
    }
  };

  const add = (raw: string) => {
    const v = raw.trim();
    if (!v) return;
    const next = Array.from(new Set([...tags, v]));
    if (next.length === tags.length) {
      setDraft("");
      return;
    }
    commit(next);
    setDraft("");
  };

  const remove = (t: string) => {
    commit(tags.filter((x) => x !== t));
  };

  const draftLower = draft.trim().toLowerCase();
  const suggestions = draftLower
    ? allTags
        .filter((t) => t.startsWith(draftLower) && !tags.includes(t))
        .slice(0, 5)
    : [];

  return (
    <div className="flex flex-wrap items-center gap-2">
      {tags.map((t) => (
        <span
          key={t}
          className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded ring-1 ${tagColor(t)}`}
        >
          {t}
          <button
            type="button"
            onClick={() => remove(t)}
            aria-label={`태그 ${t} 삭제`}
            className="opacity-60 hover:opacity-100"
          >
            <X className="size-3" />
          </button>
        </span>
      ))}
      <div className="relative">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              add(draft);
            } else if (e.key === "Backspace" && draft === "" && tags.length > 0) {
              e.preventDefault();
              remove(tags[tags.length - 1]);
            }
          }}
          placeholder="+ 태그 (소문자로 저장)"
          className="bg-transparent border border-border rounded px-2 py-0.5 text-xs w-28 focus:w-44 transition-all outline-none focus:ring-1 focus:ring-ring"
        />
        {suggestions.length > 0 && (
          <div className="absolute top-full left-0 mt-1 bg-card border border-border rounded shadow-lg z-10 min-w-32">
            {suggestions.map((s) => (
              <button
                key={s}
                type="button"
                onClick={() => add(s)}
                className="block w-full text-left px-2 py-1 text-xs hover:bg-accent"
              >
                {s}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
