import { useState } from "react";
import { Loader2, Play, Settings2 } from "lucide-react";
import { useStartJob } from "@/hooks/useJob";
import { JobOptionsDialog } from "./JobOptionsDialog";
import type {
  GraphRebuildArgs,
  IngestArgs,
  JobKind,
  JobState,
  SyncArgs,
  WikiUpdateArgs,
} from "@/lib/types";

/**
 * Command 카드 — prototype routes-misc.jsx 의 cmd 카드 패턴 (Stage 6).
 *
 * 헤더: 제목 + 상태 dot ("정상" / "오래됨" / "실패")
 * 본문: description
 * 메타: last run (relative time) + duration
 * 액션: 옵션 / 실행
 */
interface Props {
  kind: JobKind;
  label: string;
  description: string;
  /** 같은 kind 의 마지막 종료된 job (completed/failed/interrupted). 없으면 undefined. */
  lastJob?: JobState;
}

export function CommandButton({ kind, label, description, lastJob }: Props) {
  const [open, setOpen] = useState(false);
  const mutation = useStartJob(kind);

  const handleSubmit = (
    args: SyncArgs | IngestArgs | WikiUpdateArgs | GraphRebuildArgs,
  ) => {
    mutation.mutate(args);
    setOpen(false);
  };

  const status = computeStatus(lastJob);
  const lastRun = lastJob?.completed_at
    ? formatRelative(lastJob.completed_at)
    : null;
  const duration = computeDuration(lastJob);

  return (
    <>
      <div className="p-ds-4 border border-hairline rounded-lg bg-[var(--surface)] flex flex-col gap-ds-3">
        {/* 헤더 */}
        <div className="flex items-start justify-between gap-ds-2">
          <h3 className="text-t-h2 font-medium text-text">{label}</h3>
          <StatusBadge status={status} />
        </div>

        {/* 본문 */}
        <p className="text-t-meta text-text-3 leading-relaxed">{description}</p>

        {/* 메타 */}
        <div className="flex items-center gap-ds-4 text-t-meta text-text-3">
          <div className="flex flex-col">
            <span className="eyebrow">last run</span>
            <span className="font-mono text-t-mono text-text-2 mt-0.5">
              {lastRun ?? "—"}
            </span>
          </div>
          <div className="flex flex-col">
            <span className="eyebrow">duration</span>
            <span className="font-mono text-t-mono text-text-2 mt-0.5">
              {duration ?? "—"}
            </span>
          </div>
        </div>

        {/* 액션 */}
        <div className="flex items-center gap-ds-2 mt-auto pt-ds-2">
          <button
            type="button"
            onClick={() => setOpen(true)}
            disabled={mutation.isPending}
            className="inline-flex items-center gap-1 h-8 px-ds-3 rounded-md border border-border-soft text-t-meta text-text-2 hover:bg-surface-2 hover:border-border-strong transition-colors duration-fast ease-ds disabled:opacity-50"
          >
            <Settings2 className="size-3" />
            옵션
          </button>
          <button
            type="button"
            onClick={() => setOpen(true)}
            disabled={mutation.isPending}
            className="inline-flex items-center gap-1 h-8 px-ds-3 rounded-md bg-brand text-text-on-accent text-t-meta font-medium hover:bg-brand-hover transition-colors duration-fast ease-ds disabled:opacity-50 ml-auto"
          >
            {mutation.isPending ? (
              <Loader2 className="size-3 animate-spin" />
            ) : (
              <Play className="size-3 fill-current" />
            )}
            실행
          </button>
        </div>
      </div>

      <JobOptionsDialog
        kind={kind}
        open={open}
        onOpenChange={setOpen}
        onSubmit={handleSubmit}
      />
    </>
  );
}

type CardStatus = "ok" | "stale" | "failed" | "none";

function computeStatus(job: JobState | undefined): CardStatus {
  if (!job) return "none";
  if (job.status === "failed" || job.status === "interrupted") return "failed";
  if (!job.completed_at) return "ok";
  const now = Date.now();
  const completedAt = Date.parse(job.completed_at);
  if (Number.isFinite(completedAt) && now - completedAt > 24 * 60 * 60 * 1000) {
    return "stale";
  }
  return "ok";
}

function StatusBadge({ status }: { status: CardStatus }) {
  if (status === "none") {
    return (
      <span className="inline-flex items-center gap-1 text-t-caption text-text-4">
        <span className="size-1.5 rounded-full bg-[var(--text-4)]" aria-hidden />
        실행 전
      </span>
    );
  }
  const map: Record<Exclude<CardStatus, "none">, { color: string; label: string }> = {
    ok: { color: "var(--success)", label: "정상" },
    stale: { color: "var(--warn)", label: "오래됨" },
    failed: { color: "var(--danger)", label: "실패" },
  };
  const { color, label } = map[status];
  return (
    <span className="inline-flex items-center gap-1 text-t-caption text-text-3">
      <span className="size-1.5 rounded-full" style={{ background: color }} aria-hidden />
      {label}
    </span>
  );
}

function computeDuration(job: JobState | undefined): string | null {
  if (!job?.started_at || !job?.completed_at) return null;
  const start = Date.parse(job.started_at);
  const end = Date.parse(job.completed_at);
  if (!Number.isFinite(start) || !Number.isFinite(end)) return null;
  const sec = Math.max(0, Math.round((end - start) / 1000));
  if (sec < 60) return `${sec}s`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ${sec % 60}s`;
  const hr = Math.floor(min / 60);
  return `${hr}h ${min % 60}m`;
}

function formatRelative(iso: string): string {
  const t = Date.parse(iso);
  if (!Number.isFinite(t)) return iso.slice(0, 10);
  const now = Date.now();
  const diff = Math.max(0, now - t);
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day}d ago`;
  return iso.slice(0, 10);
}
