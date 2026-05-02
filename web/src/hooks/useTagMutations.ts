import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";

/**
 * 세션 태그 갱신 mutation.
 * 서버가 정규화(소문자/중복 제거)된 태그 배열을 반환한다 (`do_set_tags`).
 * onSuccess에서 sessions 리스트와 allTags 캐시를 무효화하여 자동완성 재계산을 유도한다.
 */
export function useSetTags(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (tags: string[]) => api.setTags(sessionId, tags),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sessions"] });
      qc.invalidateQueries({ queryKey: ["allTags"] });
    },
  });
}

/**
 * 즐겨찾기 토글 mutation.
 * 호출자가 낙관적 업데이트를 수행하므로 onError에서 롤백한다.
 */
export function useSetFavorite(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (favorite: boolean) => api.setFavorite(sessionId, favorite),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["sessions"] }),
  });
}

/**
 * 세션 노트 갱신 mutation (P34 Task 08).
 * NoteEditor에서 1초 debounce로 호출됨. invalidate 범위를 detail 키로 좁혀
 * sessions 리스트가 매 키 입력마다 재조회되어 깜빡이는 것을 방지.
 */
export function useSetNotes(sessionId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (notes: string | null) => api.setNotes(sessionId, notes),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["sessions", "detail", sessionId] });
    },
  });
}
