import { Loader2 } from "lucide-react";
import { useOutletContext, useParams } from "react-router";
import { MarkdownView } from "@/components/MarkdownView";
import { RelatedSessions } from "@/components/RelatedSessions";
import { SessionHeader } from "@/components/SessionHeader";
import { useSession } from "@/hooks/useSessions";
import type { SessionsOutletContext } from "./SessionsRoute";

/**
 * 세션 상세 화면.
 *
 * P32 Task 06 rework: tags / is_favorite / turn_count / start_time을 sessions 리스트 캐시
 * 대신 `/api/get` 응답 (`SessionDetail`) 에서 직접 사용. `/daily`, `/wiki`, 그래프 오버레이
 * 처럼 sessions 리스트를 거치지 않고 직접 진입한 경우에도 정확한 메타가 표시되며,
 * TagEditor / FavoriteButton 편집 시 기존 데이터 덮어쓰기 위험이 사라진다.
 */
export default function SessionDetailRoute() {
  const { id } = useParams<{ id: string }>();
  // P34 Task 02 — SessionsRoute에서 검색어를 받아 MarkdownView 하이라이트에 사용.
  // /sessions 가 아닌 다른 경로(예: /daily)에서 직접 진입하면 outlet context가 없으므로
  // optional 처리 (`useOutletContext` 가 undefined를 반환하도록).
  const outletCtx = useOutletContext<SessionsOutletContext | undefined>();
  const query = outletCtx?.query ?? "";
  const { data, isLoading, error } = useSession(id, true);

  if (isLoading) {
    return (
      <div className="p-8 flex items-center text-muted-foreground text-sm">
        <Loader2 className="size-4 animate-spin mr-2" /> 세션 불러오는 중…
      </div>
    );
  }
  if (error) {
    return (
      <div className="p-8 text-rose-400 text-sm whitespace-pre-wrap">
        {error instanceof Error ? error.message : String(error)}
      </div>
    );
  }
  if (!data || !id) return null;

  const body = data.content ?? "";

  return (
    <div className="p-6 max-w-4xl">
      <SessionHeader id={id} detail={data} />
      {body ? (
        <MarkdownView content={body} query={query} />
      ) : (
        <div className="text-sm text-muted-foreground italic">
          본문이 비어 있습니다. (vault 파일 없음 · turns 없음)
        </div>
      )}
      <RelatedSessions sessionId={id} />
    </div>
  );
}
