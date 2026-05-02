import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type {
  IngestArgs,
  JobKind,
  JobState,
  SyncArgs,
  WikiUpdateArgs,
} from "@/lib/types";

/**
 * 활성 job 폴링 (5초). SSE는 단건 스트림이라 신규 job 등장은 폴링으로 감지한다.
 * Task 06 글로벌 배너에서도 동일 query key를 공유.
 */
export function useActiveJobs() {
  return useQuery({
    queryKey: ["jobs", "active"],
    queryFn: () => api.listActiveJobs(),
    refetchInterval: 5000,
  });
}

/**
 * 최근 완료 job 목록 (10건). CommandsRoute 하단 표시용.
 * 활성 job 변경 시(invalidate) 함께 갱신된다.
 */
export function useRecentJobs(limit: number = 10) {
  return useQuery({
    queryKey: ["jobs", "recent", limit],
    queryFn: () => api.listRecentJobs(limit),
    refetchInterval: 30_000,
  });
}

/**
 * 단일 job 조회 (SSE가 끊겼을 때 폴링 fallback).
 * 완료된 job은 더 이상 폴링하지 않는다.
 */
export function useJob(id: string | undefined) {
  return useQuery({
    queryKey: ["jobs", "detail", id],
    queryFn: () => api.getJob(id!),
    enabled: !!id,
    refetchInterval: (q) => {
      const data = q.state.data as JobState | undefined;
      if (!data) return false;
      const terminal =
        data.status === "completed" ||
        data.status === "failed" ||
        data.status === "interrupted";
      return terminal ? false : 3000;
    },
  });
}

type JobArgs = SyncArgs | IngestArgs | WikiUpdateArgs;

/**
 * Job 시작 mutation. kind에 따라 적절한 시작 엔드포인트로 분기.
 *
 * - 성공 시 jobs 캐시 invalidate + toast.
 * - 409 Conflict (다른 mutating job 실행 중)은 Error.message에 "HTTP 409"가 포함된다 (jfetch 동작).
 */
export function useStartJob(kind: JobKind) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (args: JobArgs) => {
      switch (kind) {
        case "sync":
          return api.startSync(args as SyncArgs);
        case "ingest":
          return api.startIngest(args as IngestArgs);
        case "wiki_update":
          return api.startWikiUpdate(args as WikiUpdateArgs);
      }
    },
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ["jobs"] });
      toast.success(`${kind} 시작됨 (${data.job_id.slice(0, 8)})`);
    },
    onError: (err) => {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("409")) {
        toast.error("이미 실행 중인 작업이 있습니다");
      } else {
        toast.error(`작업 시작 실패: ${msg}`);
      }
    },
  });
}

/**
 * Job 취소 mutation. POST /api/jobs/{id}/cancel 호출.
 *
 * - onSuccess 시 ["jobs"] (active/recent) 와 ["job", jobId] 캐시 invalidate.
 * - onError 는 콘솔 로그만 (sonner toast 통합은 별도 task).
 * - 백엔드(Task 01)가 NOT_IMPLEMENTED 상태여도 graceful 처리.
 */
export function useCancelJob() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => api.cancelJob(jobId),
    onSuccess: (_data, jobId) => {
      qc.invalidateQueries({ queryKey: ["jobs"] });
      qc.invalidateQueries({ queryKey: ["job", jobId] });
    },
    onError: (err) => {
      console.error("[useCancelJob] cancel failed:", err);
    },
  });
}
