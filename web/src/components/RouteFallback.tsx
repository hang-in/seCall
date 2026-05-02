import { Loader2 } from "lucide-react";

/**
 * React.lazy로 코드 분할된 라우트 컴포넌트의 chunk fetch 동안 표시.
 * 다른 로딩 표시(`SessionDetailRoute`의 "세션 불러오는 중…" 등)와 구분되도록 단순한 형태.
 */
export function RouteFallback() {
  return (
    <div className="p-8 flex items-center justify-center text-muted-foreground text-sm">
      <Loader2 className="size-4 animate-spin mr-2" /> 화면 로드 중…
    </div>
  );
}
