import { Loader2 } from "lucide-react";

/**
 * lazy chunk fetch 동안 표시 (Stage 3).
 */
export function RouteFallback() {
  return (
    <div className="p-ds-7 flex items-center justify-center text-t-small text-text-3">
      <Loader2 className="size-4 animate-spin mr-ds-2" /> 화면 로드 중…
    </div>
  );
}
