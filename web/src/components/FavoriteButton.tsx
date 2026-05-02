import { Star } from "lucide-react";
import { useEffect, useState } from "react";
import { useSetFavorite } from "@/hooks/useTagMutations";

interface Props {
  sessionId: string;
  initial: boolean;
}

/**
 * 즐겨찾기 별표 토글.
 * 낙관적 업데이트 후 mutation 실패 시 이전 상태로 롤백한다.
 * `initial`이 props로 바뀌면 (예: 다른 세션으로 이동) 내부 상태도 동기화한다.
 */
export function FavoriteButton({ sessionId, initial }: Props) {
  const [on, setOn] = useState(initial);
  const mutation = useSetFavorite(sessionId);

  useEffect(() => {
    setOn(initial);
  }, [initial, sessionId]);

  return (
    <button
      type="button"
      onClick={() => {
        const prev = on;
        const next = !on;
        setOn(next); // 낙관적
        mutation.mutate(next, {
          onError: () => setOn(prev),
        });
      }}
      aria-label={on ? "즐겨찾기 해제" : "즐겨찾기"}
      aria-pressed={on}
      data-hotkey="favorite"
      className="p-2 rounded hover:bg-accent transition-colors"
    >
      <Star
        className={`size-5 ${
          on ? "fill-amber-400 text-amber-400" : "text-muted-foreground"
        }`}
      />
    </button>
  );
}
