import { Loader2 } from "lucide-react";
import { useNavigate, useParams } from "react-router";
import { SessionListItem } from "./SessionListItem";
import { useListHotkeys } from "@/hooks/useListHotkeys";
import { useSemanticRecall, useSessionsList } from "@/hooks/useSessions";
import type {
  RecallResultItem,
  SearchMode,
  SessionFilterState,
  SessionListItem as Session,
} from "@/lib/types";

interface Props {
  query: string;
  mode: SearchMode;
  filters: SessionFilterState;
  /** 디폴트 100. 무한 스크롤은 Phase 1. */
  pageSize?: number;
}

/**
 * `RecallResultItem` (turn 단위 + flat metadata) → `SessionListItem` 호환 객체.
 * 같은 session_id가 여러 turn으로 나오면 첫 번째 매칭만 남긴다 (클라이언트 안전망 — 서버 diversify_by_session 보완).
 *
 * snippet은 turn 본문이라 SessionListItem.summary에 그대로 넣으면 의미가 다르지만,
 * 시맨틱 결과는 "어떤 turn이 매칭됐는지"를 보여주는 게 더 가치가 있어 summary 자리에 표시.
 */
function recallToSessions(items: RecallResultItem[]): Session[] {
  const seen = new Set<string>();
  const out: Session[] = [];
  for (const r of items) {
    if (seen.has(r.session_id)) continue;
    seen.add(r.session_id);
    out.push({
      id: r.session_id,
      agent: r.metadata.agent,
      project: r.metadata.project,
      model: r.metadata.model,
      date: r.metadata.date,
      // 백엔드가 SearchResult에 start_time을 포함하지 않음 → date를 사용 (시간 정밀도 손실 허용).
      start_time: r.metadata.date,
      turn_count: 0,
      summary: r.snippet || null,
      tags: [],
      is_favorite: false,
      session_type: r.metadata.session_type,
      vault_path: r.metadata.vault_path,
    });
  }
  return out;
}

export function SessionList({ query, mode, filters, pageSize = 100 }: Props) {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const trimmed = query.trim();

  // 시맨틱 모드 + 비어있지 않은 query에서만 recall 호출. 그 외엔 keyword 리스트.
  const useSemantic = mode === "semantic" && trimmed.length > 0;

  // useSessionsList는 항상 enabled (기존 시그니처 유지). semantic 모드에서는 그 결과를 사용 안 함.
  // 별도 enabled 토글이 필요하면 Task 03에서 useSessionsList 시그니처 확장 시 정리.
  const keywordList = useSessionsList({
    q: trimmed === "" ? undefined : trimmed,
    page: 1,
    page_size: pageSize,
    ...filters,
  });

  const semanticList = useSemanticRecall(query, filters, {
    enabled: useSemantic,
  });

  // P34 Task 04 — 리스트 단축키 (j/k/Enter/[/]) 등록.
  // 모드에 따라 활성 리스트가 다르므로, 현재 화면에 보이는 항목을 단축키 대상에 매핑.
  // useHotkeys 자체는 항상 호출되어야 하므로(Rules of Hooks), 빈 배열도 그대로 넘긴다.
  const hotkeyItems: Session[] = useSemantic
    ? semanticList.data
      ? recallToSessions(semanticList.data.results)
      : []
    : (keywordList.data?.items ?? []);
  useListHotkeys(hotkeyItems, id, (sid) =>
    navigate(`/sessions/${encodeURIComponent(sid)}`),
  );

  if (useSemantic) {
    if (semanticList.isLoading) {
      return (
        <div className="flex items-center justify-center p-8 text-muted-foreground text-sm">
          <Loader2 className="size-4 animate-spin mr-2" /> 시맨틱 검색 중…
        </div>
      );
    }
    if (semanticList.isError) {
      const msg =
        semanticList.error instanceof Error
          ? semanticList.error.message
          : String(semanticList.error);
      return (
        <div className="p-6 text-rose-400 text-sm whitespace-pre-wrap">
          시맨틱 검색 실패: {msg}
        </div>
      );
    }
    const data = semanticList.data;
    if (!data || data.count === 0) {
      // 백엔드는 Ollama 미설치/embedding 비활성 시 빈 결과만 반환 (에러 throw 안 함).
      // → count === 0 + query 있음 = 비활성 가능성 안내.
      return (
        <div className="p-8 text-muted-foreground text-sm text-center space-y-2">
          <div>매칭되는 결과가 없습니다.</div>
          <div className="text-xs">
            시맨틱 검색이 비활성 상태일 수 있습니다 (Ollama 필요)
          </div>
        </div>
      );
    }
    const sessions = recallToSessions(data.results);
    return (
      <div>
        {semanticList.isFetching && (
          <div className="px-3 py-1 text-[10px] text-muted-foreground border-b border-border">
            업데이트 중…
          </div>
        )}
        <div className="divide-y divide-border">
          {sessions.map((s, idx) => {
            const score = data.results[idx]?.score;
            return (
              <div key={s.id} className="relative">
                <SessionListItem
                  session={s}
                  query={query}
                  selected={s.id === id}
                  onSelect={() =>
                    navigate(`/sessions/${encodeURIComponent(s.id)}`)
                  }
                />
                {typeof score === "number" && (
                  <span className="absolute right-3 bottom-2 text-[10px] text-muted-foreground tabular-nums pointer-events-none">
                    score {score.toFixed(2)}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  }

  // ── keyword 모드 (기존 흐름) ───────────────────────────────
  const { data, isLoading, isError, error, isFetching } = keywordList;

  if (isLoading) {
    return (
      <div className="flex items-center justify-center p-8 text-muted-foreground text-sm">
        <Loader2 className="size-4 animate-spin mr-2" /> 불러오는 중…
      </div>
    );
  }

  if (isError) {
    return (
      <div className="p-6 text-rose-400 text-sm whitespace-pre-wrap">
        세션 로드 실패: {error instanceof Error ? error.message : String(error)}
      </div>
    );
  }

  if (!data || data.items.length === 0) {
    return (
      <div className="p-8 text-muted-foreground text-sm text-center">
        조건에 맞는 세션이 없습니다.
      </div>
    );
  }

  return (
    <div>
      {isFetching && (
        <div className="px-3 py-1 text-[10px] text-muted-foreground border-b border-border">
          업데이트 중…
        </div>
      )}
      <div className="divide-y divide-border">
        {data.items.map((s) => (
          <SessionListItem
            key={s.id}
            session={s}
            query={query}
            selected={s.id === id}
            onSelect={() => navigate(`/sessions/${encodeURIComponent(s.id)}`)}
          />
        ))}
      </div>
      {data.total > data.items.length && (
        <div className="p-3 text-[11px] text-muted-foreground text-center border-t border-border">
          {data.items.length} / {data.total} (페이지네이션 Phase 1)
        </div>
      )}
    </div>
  );
}
