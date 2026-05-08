import { useMemo } from "react";
import { Activity, History, Play } from "lucide-react";
import { Card } from "@/components/ui/card";
import { useActiveJobs, useRecentJobs } from "@/hooks/useJob";
import { CommandButton } from "@/components/CommandButton";
import { JobItem } from "@/components/JobItem";
import type { JobKind, JobState } from "@/lib/types";

export default function CommandsRoute() {
  const { data: active } = useActiveJobs();
  const { data: recent } = useRecentJobs(20);

  const activeJobs = active?.jobs ?? [];
  const recentJobs = (recent?.jobs ?? []).filter(
    // active와 중복 제거 (백엔드가 이미 분리해서 주지만 안전장치)
    (j) => !activeJobs.some((a) => a.id === j.id),
  );

  // kind 별 가장 최근 종료(완료/실패/취소) job. CommandButton 의 last run / status 표기에 사용.
  const lastByKind = useMemo<Partial<Record<JobKind, JobState>>>(() => {
    const map: Partial<Record<JobKind, JobState>> = {};
    for (const j of recentJobs) {
      const cur = map[j.kind];
      const ja = j.completed_at ? Date.parse(j.completed_at) : 0;
      const cb = cur?.completed_at ? Date.parse(cur.completed_at) : 0;
      if (!cur || ja > cb) map[j.kind] = j;
    }
    return map;
  }, [recentJobs]);

  return (
    <div className="p-ds-6 max-w-4xl mx-auto space-y-ds-6 overflow-auto h-full">
      <header className="space-y-ds-1">
        <h1 className="text-t-display-s font-medium tracking-tight flex items-center gap-ds-2">
          <Play className="size-5 text-text-3" /> Commands
        </h1>
        <p className="text-t-small text-text-3">
          명령을 실행하여 sync / ingest / wiki update / graph rebuild 를 수행합니다.
          한 번에 하나의 mutating 작업만 실행 가능합니다.
        </p>
      </header>

      <section className="space-y-ds-3">
        <h2 className="eyebrow">새 작업</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-ds-3">
          <CommandButton
            kind="sync"
            label="Sync"
            description="git pull → reindex → ingest → push 한 번에 실행."
            lastJob={lastByKind.sync}
          />
          <CommandButton
            kind="ingest"
            label="Ingest"
            description="새 세션 파싱 + BM25/벡터 인덱싱."
            lastJob={lastByKind.ingest}
          />
          <CommandButton
            kind="wiki_update"
            label="Wiki Update"
            description="LLM 으로 vault/wiki 페이지 갱신."
            lastJob={lastByKind.wiki_update}
          />
          <CommandButton
            kind="graph_rebuild"
            label="Graph Rebuild"
            description="시맨틱 그래프 재구축 (since · session · all · retry-failed 옵션)."
            lastJob={lastByKind.graph_rebuild}
          />
        </div>
      </section>

      <Card className="p-ds-4 space-y-ds-3 border-hairline">
        <h2 className="text-t-h2 font-medium flex items-center gap-ds-2">
          <Activity className="size-4 text-text-3" />
          현재 활성 작업 ({activeJobs.length})
        </h2>
        {activeJobs.length ? (
          <div className="space-y-ds-2">
            {activeJobs.map((j) => (
              <JobItem key={j.id} job={j} />
            ))}
          </div>
        ) : (
          <div className="text-t-small text-text-3">활성 작업 없음</div>
        )}
      </Card>

      <Card className="p-ds-4 space-y-ds-3 border-hairline">
        <h2 className="text-t-h2 font-medium flex items-center gap-ds-2">
          <History className="size-4 text-text-3" />
          최근 작업 ({recentJobs.length})
        </h2>
        {recentJobs.length ? (
          <div className="space-y-ds-2">
            {recentJobs.slice(0, 10).map((j) => (
              <JobItem key={j.id} job={j} />
            ))}
          </div>
        ) : (
          <div className="text-t-small text-text-3">최근 작업 없음</div>
        )}
      </Card>
    </div>
  );
}
