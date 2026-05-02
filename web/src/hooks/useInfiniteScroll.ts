import { useEffect, useRef } from "react";

/**
 * sentinel 엘리먼트가 viewport 안에 들어오면 onIntersect 호출.
 *
 * - rootMargin "200px 0px": 끝에서 200px 전에 미리 fetch (체감 끊김 감소)
 * - hasMore=false면 observer 미설정 → 마지막 페이지 도달 후 호출 안 됨
 * - enabled=false (예: isFetching 중)면 일시 중단
 *
 * 모던 브라우저는 IntersectionObserver를 모두 지원하므로 폴백 불필요.
 */
export function useInfiniteScroll(opts: {
  onIntersect: () => void;
  hasMore: boolean;
  enabled?: boolean;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const { onIntersect, hasMore, enabled = true } = opts;

  useEffect(() => {
    const el = ref.current;
    if (!el || !hasMore || !enabled) return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            onIntersect();
            break;
          }
        }
      },
      { rootMargin: "200px 0px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [onIntersect, hasMore, enabled]);

  return ref;
}
