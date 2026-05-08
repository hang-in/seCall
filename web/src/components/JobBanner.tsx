import { useNavigate } from "react-router";
import { ChevronRight, Loader2, X } from "lucide-react";
import { useActiveJobs, useCancelJob } from "@/hooks/useJob";
import type { JobKind } from "@/lib/types";

/**
 * 글로벌 상단 진행 배너 — Calm/Editorial 톤 (Stage 2b).
 *
 * - 활성 job 이 있을 때만 렌더 (없으면 null).
 * - 단일 큐 정책상 활성 job 은 보통 1개. N개 표시 패턴은 미래 확장용.
 * - 첫 번째 job 의 phase/progress 노출 + cancel + "보기" 라우트 점프.
 */
export function JobBanner() {
  const { data } = useActiveJobs();
  const navigate = useNavigate();

  if (!data?.jobs.length) return null;

  const first = data.jobs[0];
  const pct =
    typeof first.progress === "number"
      ? Math.round(first.progress * 100)
      : null;

  return (
    <div className="border-b border-hairline bg-[var(--surface)]">
      <div className="px-ds-4 py-ds-2 flex items-center gap-ds-3">
        <span
          className="size-1.5 shrink-0 rounded-full bg-brand animate-pulse"
          aria-hidden
        />
        <div className="flex items-baseline gap-ds-2 min-w-0">
          <span className="text-t-body font-medium text-text shrink-0">
            {labelFor(first.kind)}
          </span>
          {first.current_phase && (
            <span className="text-t-meta text-text-3 truncate">
              {first.current_phase}
            </span>
          )}
          {data.jobs.length > 1 && (
            <span className="text-t-meta text-text-4 shrink-0">
              · 외 {data.jobs.length - 1}건
            </span>
          )}
        </div>

        {pct !== null && (
          <>
            <div className="flex-1 max-w-[180px] h-1 rounded-full bg-surface-3 overflow-hidden">
              <div
                className="h-full bg-brand transition-all duration-base ease-ds"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="font-mono text-t-mono text-text-3 tabular-nums w-9 text-right shrink-0">
              {pct}%
            </span>
          </>
        )}
        {pct === null && <div className="flex-1" />}

        <CancelButton jobId={first.id} kind={first.kind} />
        <button
          type="button"
          onClick={() => navigate("/commands")}
          className="inline-flex items-center gap-1 text-t-meta text-text-3 hover:text-text transition-colors duration-fast ease-ds"
        >
          보기 <ChevronRight className="size-3" />
        </button>
      </div>
    </div>
  );
}

function labelFor(kind: JobKind): string {
  switch (kind) {
    case "sync":
      return "Sync";
    case "ingest":
      return "Ingest";
    case "wiki_update":
      return "Wiki Update";
    case "graph_rebuild":
      return "Graph Rebuild";
    default:
      return kind;
  }
}

/**
 * 배너용 취소 버튼. 첫 번째 활성 job 한정.
 * confirm 후 useCancelJob 발화. pending 시 disabled + 로더.
 * 다중 active job 은 단일 큐 정책상 거의 발생하지 않으며, 개별 취소는 JobItem 에서 가능.
 */
function CancelButton({ jobId, kind }: { jobId: string; kind: JobKind }) {
  const cancel = useCancelJob();

  const onClick = () => {
    if (cancel.isPending) return;
    if (!window.confirm(`이 ${labelFor(kind)} 작업을 취소하시겠습니까?`)) return;
    cancel.mutate(jobId);
  };

  return (
    <button
      type="button"
      onClick={onClick}
      disabled={cancel.isPending}
      className="inline-flex items-center gap-1 h-6 px-ds-2 rounded-md border border-border-soft text-t-meta text-text-2 hover:bg-surface-2 hover:border-border-strong disabled:opacity-50 transition-colors duration-fast ease-ds"
    >
      {cancel.isPending ? (
        <>
          <Loader2 className="size-3 animate-spin" />
          취소 중…
        </>
      ) : (
        <>
          <X className="size-3" />
          취소
        </>
      )}
    </button>
  );
}
