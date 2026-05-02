import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { JobState } from "@/lib/types";

/**
 * 모든 페이지에서 recent jobs를 폴링 → 상태 변화 (active → completed/failed/interrupted) 감지 시 toast 발행.
 *
 * - Layout에서 단 한 번만 호출되어야 한다 (전역 listener).
 * - 폴링 간격은 useActiveJobs와 동일한 5초.
 * - 첫 로드는 toast를 발행하지 않는다 (페이지 진입 시 이미 종료된 job들로 spam되는 것을 방지).
 *   첫 데이터 도착 시 현재 상태 스냅샷만 남기고 이후 변화부터 알림.
 * - 메모리 기반(useRef) 상태 추적이므로 페이지 reload 시 초기화된다 (의도된 동작).
 *   ⇒ 서버 재시작 후 reload 시 interrupted 상태 자체는 첫 로드라 알림이 안 가지만,
 *      이후 상태가 다시 바뀌는 경우는 정상 감지된다.
 */
export function useJobLifecycle() {
  const previous = useRef<Map<string, string>>(new Map());
  const initialized = useRef(false);
  const qc = useQueryClient();

  const { data } = useQuery({
    queryKey: ["jobs", "lifecycle"],
    queryFn: () => api.listRecentJobs(20),
    refetchInterval: 5000,
  });

  useEffect(() => {
    if (!data?.jobs) return;

    if (!initialized.current) {
      data.jobs.forEach((job) => previous.current.set(job.id, job.status));
      initialized.current = true;
      return;
    }

    for (const job of data.jobs) {
      const prev = previous.current.get(job.id);
      if (prev && prev !== job.status) {
        if (job.status === "completed") {
          toast.success(`${job.kind} 완료`, {
            description: summarizeResult(job),
            action: {
              label: "보기",
              onClick: () => qc.invalidateQueries({ queryKey: ["sessions"] }),
            },
          });
        } else if (job.status === "failed") {
          toast.error(`${job.kind} 실패`, {
            description: job.error ?? "unknown error",
          });
        } else if (job.status === "interrupted") {
          toast.warning(`${job.kind} 중단됨`, {
            description: "서버 재시작으로 작업이 중단되었습니다",
          });
        }
      }
      previous.current.set(job.id, job.status);
    }
  }, [data, qc]);
}

function summarizeResult(job: JobState): string | undefined {
  if (!job.result || typeof job.result !== "object") return undefined;
  const r = job.result as Record<string, unknown>;
  if (job.kind === "sync") {
    const parts: string[] = [];
    if (typeof r.pulled === "number") parts.push(`pulled=${r.pulled}`);
    if (typeof r.reindexed === "number") parts.push(`reindexed=${r.reindexed}`);
    if (typeof r.ingested === "number") parts.push(`ingested=${r.ingested}`);
    if (r.partial_failure) parts.push(`(부분 성공)`);
    return parts.length > 0 ? parts.join(" / ") : undefined;
  }
  if (job.kind === "ingest") {
    return `${r.ingested ?? 0}개 신규 세션`;
  }
  if (job.kind === "wiki_update") {
    return `${r.pages_written ?? 0}개 위키 페이지 갱신`;
  }
  return undefined;
}
