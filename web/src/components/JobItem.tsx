import { useEffect, useState } from "react";
import { Loader2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useCancelJob } from "@/hooks/useJob";
import { useJobStream } from "@/hooks/useJobStream";
import type {
  GraphRebuildOutcome,
  IngestOutcome,
  JobState,
  JobStatus,
  ProgressEvent,
  SyncOutcome,
  WikiOutcome,
} from "@/lib/types";

/**
 * 단일 job 카드.
 * - props로 들어온 JobState를 초기값으로 사용
 * - 미완료 job은 SSE 구독으로 실시간 갱신
 * - 외부 polling으로 prop이 갱신될 수 있어 status가 terminal이면 prop 우선 적용
 */
export function JobItem({ job: initial }: { job: JobState }) {
  const [job, setJob] = useState<JobState>(initial);

  // 부모 polling으로 prop이 변하면 동기화 (단, SSE로 받은 더 최신 상태를 덮어쓰지 않도록 status 비교)
  useEffect(() => {
    setJob((prev) => {
      if (prev.id !== initial.id) return initial;
      // 내가 받은 SSE가 이미 terminal이면 그대로 둠
      if (isTerminal(prev.status) && !isTerminal(initial.status)) return prev;
      return initial;
    });
  }, [initial]);

  const enabled = !isTerminal(job.status);
  useJobStream(enabled ? job.id : undefined, (e) => {
    setJob((prev) => applyEvent(prev, e));
  }, enabled);

  const outcome = renderOutcome(job);
  const isActive = job.status === "started" || job.status === "running";

  return (
    <div className="border border-border rounded p-3 space-y-2 bg-card">
      <div className="flex items-center justify-between text-sm">
        <div className="flex items-center gap-2">
          <span className="font-medium">{job.kind}</span>
          <StatusBadge status={job.status} />
        </div>
        <div className="flex items-center gap-2">
          {isActive && <CancelButton jobId={job.id} kind={job.kind} />}
          <span className="font-mono text-xs opacity-60">
            {job.id.slice(0, 8)}
          </span>
        </div>
      </div>
      {job.current_phase && (
        <div className="text-xs text-muted-foreground">
          phase:{" "}
          <span className="font-medium text-foreground">
            {job.current_phase}
          </span>
          {typeof job.progress === "number" && (
            <span> · {Math.round(job.progress * 100)}%</span>
          )}
        </div>
      )}
      {typeof job.progress === "number" && (
        <div className="h-1 w-full bg-muted rounded-full overflow-hidden">
          <div
            className="h-full bg-primary transition-all"
            style={{ width: `${Math.min(100, job.progress * 100)}%` }}
          />
        </div>
      )}
      {job.message && (
        <div className="text-xs whitespace-pre-wrap opacity-80">
          {job.message}
        </div>
      )}
      {job.error && (
        <div className="text-xs text-rose-400 whitespace-pre-wrap">
          {job.error}
        </div>
      )}
      {outcome && (
        <div className="text-xs text-muted-foreground border-t border-border pt-2">
          {outcome}
        </div>
      )}
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span className="tabular-nums">{formatTime(job.started_at)}</span>
        {job.completed_at && (
          <>
            <span>→</span>
            <span className="tabular-nums">
              {formatTime(job.completed_at)}
            </span>
          </>
        )}
      </div>
    </div>
  );
}

// ----------------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------------

function isTerminal(status: JobStatus): boolean {
  return (
    status === "completed" ||
    status === "failed" ||
    status === "interrupted"
  );
}

function applyEvent(prev: JobState, e: ProgressEvent): JobState {
  switch (e.type) {
    case "initial_state":
      // 재접속 시 서버 스냅샷으로 완전 교체
      return e.state;
    case "phase_start":
      return {
        ...prev,
        status: "running",
        current_phase: e.phase,
        progress: null,
      };
    case "phase_complete":
      return { ...prev, message: `${e.phase} 완료` };
    case "message":
      return { ...prev, message: e.text };
    case "progress":
      return { ...prev, progress: e.ratio };
    case "done":
      return {
        ...prev,
        status: "completed",
        result: e.result,
        progress: 1,
        completed_at: new Date().toISOString(),
      };
    case "failed":
      return {
        ...prev,
        status: "failed",
        error: e.error,
        result: e.partial_result ?? prev.result,
        completed_at: new Date().toISOString(),
      };
  }
}

/**
 * 취소 버튼. status가 active(started/running)일 때만 마운트되며,
 * confirm 후 useCancelJob mutation 발화. pending 동안 disabled + 로더 표시.
 * 성공 시 status가 interrupted/failed 등으로 갱신되면 부모가 언마운트.
 */
function CancelButton({
  jobId,
  kind,
}: {
  jobId: string;
  kind: JobState["kind"];
}) {
  const cancel = useCancelJob();

  const onClick = () => {
    if (cancel.isPending) return;
    if (!window.confirm(`이 ${kind} 작업을 취소하시겠습니까?`)) return;
    cancel.mutate(jobId);
  };

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={onClick}
      disabled={cancel.isPending}
      className="h-7 px-2 text-xs"
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
    </Button>
  );
}

function StatusBadge({ status }: { status: JobStatus }) {
  const variant = (() => {
    switch (status) {
      case "completed":
        return "secondary";
      case "failed":
        return "destructive";
      case "interrupted":
        return "outline";
      case "running":
      case "started":
      default:
        return "default";
    }
  })() as "default" | "secondary" | "destructive" | "outline";

  return (
    <Badge variant={variant} className="text-[10px] px-1.5 py-0">
      {status}
    </Badge>
  );
}

function formatTime(iso: string): string {
  // ISO를 표시용으로 단순화 (HH:mm:ss)
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleTimeString("ko-KR", { hour12: false });
  } catch {
    return iso;
  }
}

function renderOutcome(job: JobState): string | null {
  if (!isTerminal(job.status) || !job.result) return null;
  switch (job.kind) {
    case "sync": {
      const r = job.result as SyncOutcome;
      const parts: string[] = [];
      if (r.pulled) parts.push("pulled");
      if (typeof r.reindexed === "number" && r.reindexed > 0)
        parts.push(`reindexed=${r.reindexed}`);
      parts.push(`ingested=${r.ingested}`);
      if (r.wiki_updated) parts.push("wiki ok");
      if (r.pushed) parts.push("pushed");
      if (r.graph_nodes_added)
        parts.push(`+${r.graph_nodes_added}n/${r.graph_edges_added ?? 0}e`);
      if (r.partial_failure) parts.push("partial");
      return parts.join(" · ");
    }
    case "ingest": {
      const r = job.result as IngestOutcome;
      const parts = [
        `ingested=${r.ingested}`,
        `skipped=${r.skipped}`,
      ];
      if (r.errors) parts.push(`errors=${r.errors}`);
      if (r.skipped_min_turns)
        parts.push(`min_turns_skip=${r.skipped_min_turns}`);
      if (r.hook_failures) parts.push(`hook_fail=${r.hook_failures}`);
      if (r.graph_nodes_added)
        parts.push(`+${r.graph_nodes_added}n/${r.graph_edges_added}e`);
      return parts.join(" · ");
    }
    case "wiki_update": {
      const r = job.result as WikiOutcome;
      return `${r.backend} → ${r.target} · ${r.pages_written} pages`;
    }
    case "graph_rebuild": {
      const r = job.result as GraphRebuildOutcome;
      const parts = [
        `processed=${r.processed}`,
        `succeeded=${r.succeeded}`,
      ];
      if (r.failed) parts.push(`failed=${r.failed}`);
      if (r.skipped) parts.push(`skipped=${r.skipped}`);
      if (r.edges_added) parts.push(`+${r.edges_added}e`);
      return parts.join(" · ");
    }
  }
}
