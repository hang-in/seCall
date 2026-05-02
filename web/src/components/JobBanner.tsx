import { useNavigate } from "react-router";
import { ChevronRight, Loader2 } from "lucide-react";
import { useActiveJobs } from "@/hooks/useJob";

/**
 * 글로벌 상단 진행 배너.
 *
 * - 활성 job이 있을 때만 렌더한다 (없으면 null).
 * - 단일 큐 정책상 활성 job은 보통 1개지만, 미래 확장(병렬 큐)을 위해 N개 표시 패턴을 유지한다.
 * - 첫 번째 job의 phase/progress를 부가정보로 노출.
 * - "보기" 버튼은 `/commands` 라우트로 이동시켜 상세 진행 상태를 확인하게 한다.
 */
export function JobBanner() {
  const { data } = useActiveJobs();
  const navigate = useNavigate();

  if (!data?.jobs.length) return null;

  const first = data.jobs[0];
  return (
    <div className="border-b border-border bg-accent/40 px-4 py-2 flex items-center gap-3 text-sm">
      <Loader2 className="size-4 animate-spin text-primary" />
      <div className="flex-1 min-w-0">
        <span className="font-medium">{data.jobs.length}개 작업 실행 중</span>
        {first.current_phase && (
          <span className="text-muted-foreground ml-2">
            · {first.kind} / {first.current_phase}
            {typeof first.progress === "number" && (
              <span> ({Math.round(first.progress * 100)}%)</span>
            )}
          </span>
        )}
      </div>
      <button
        onClick={() => navigate("/commands")}
        className="flex items-center gap-1 text-xs hover:underline"
      >
        보기 <ChevronRight className="size-3" />
      </button>
    </div>
  );
}
