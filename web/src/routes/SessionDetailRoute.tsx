import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { useOutletContext, useParams } from "react-router";
import { MarkdownView } from "@/components/MarkdownView";
import { SessionAside } from "@/components/SessionAside";
import { SessionDetailHead } from "@/components/SessionDetailHead";
import { useSession } from "@/hooks/useSessions";
import type { SessionsOutletContext } from "./SessionsRoute";

/**
 * 세션 상세 — prototype route-sessions.jsx 의 SessionDetailPane 2-column 패턴 (Stage 7).
 *
 * - main: SessionDetailHead + MarkdownView
 * - aside: SessionAside (메타 KV / 미니차트 / Related / Notes 4 카드)
 *
 * 작은 화면(<1024px) 에서는 column 이 1열로 stack.
 */
export default function SessionDetailRoute() {
  const { id } = useParams<{ id: string }>();
  const outletCtx = useOutletContext<SessionsOutletContext | undefined>();
  const query = outletCtx?.query ?? "";
  const { data, isLoading, error } = useSession(id, true);

  // R2 — "턴 구분 표시" 토글. 기본 OFF, localStorage 에 저장해 세션 이동해도 유지.
  const [showTurnHeaders, setShowTurnHeaders] = useState<boolean>(
    () =>
      typeof window !== "undefined" &&
      window.localStorage.getItem("secall.showTurnHeaders") === "1",
  );
  useEffect(() => {
    window.localStorage.setItem(
      "secall.showTurnHeaders",
      showTurnHeaders ? "1" : "0",
    );
  }, [showTurnHeaders]);

  if (isLoading) {
    return (
      <div className="p-ds-7 flex items-center text-t-small text-text-3">
        <Loader2 className="size-4 animate-spin mr-ds-2" /> 세션 불러오는 중…
      </div>
    );
  }
  if (error) {
    return (
      <div className="p-ds-7 text-t-small text-status-danger whitespace-pre-wrap">
        {error instanceof Error ? error.message : String(error)}
      </div>
    );
  }
  if (!data || !id) return null;

  const body = data.content ?? "";

  return (
    <div className="p-ds-6 grid grid-cols-1 lg:grid-cols-[minmax(0,var(--read-w))_minmax(0,300px)] gap-ds-6 max-w-[1100px]">
      <div className="min-w-0">
        <SessionDetailHead id={id} detail={data} />
        {body ? (
          <>
            <div className="flex justify-end mb-ds-3">
              <label className="inline-flex items-center gap-ds-2 text-t-meta text-text-3 cursor-pointer select-none hover:text-text transition-colors duration-fast ease-ds">
                <input
                  type="checkbox"
                  checked={showTurnHeaders}
                  onChange={(e) => setShowTurnHeaders(e.target.checked)}
                  className="size-3.5 accent-brand cursor-pointer"
                />
                턴 구분 표시
              </label>
            </div>
            <MarkdownView
              content={body}
              query={query}
              showTurnHeaders={showTurnHeaders}
            />
          </>
        ) : (
          <div className="text-t-small text-text-3 italic">
            본문이 비어 있습니다. (vault 파일 없음 · turns 없음)
          </div>
        )}
      </div>
      <SessionAside sessionId={id} detail={data} />
    </div>
  );
}
