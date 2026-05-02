import { Activity, History, Play } from "lucide-react";
import { Card } from "@/components/ui/card";
import { useActiveJobs, useRecentJobs } from "@/hooks/useJob";
import { CommandButton } from "@/components/CommandButton";
import { JobItem } from "@/components/JobItem";

export default function CommandsRoute() {
  const { data: active } = useActiveJobs();
  const { data: recent } = useRecentJobs(10);

  const activeJobs = active?.jobs ?? [];
  const recentJobs = (recent?.jobs ?? []).filter(
    // active와 중복 제거 (백엔드가 이미 분리해서 주지만 안전장치)
    (j) => !activeJobs.some((a) => a.id === j.id),
  );

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-6 overflow-auto h-full">
      <header>
        <h1 className="text-2xl font-semibold flex items-center gap-2">
          <Play className="size-5" /> Commands
        </h1>
        <p className="text-sm text-muted-foreground mt-1">
          명령을 실행하여 sync / ingest / wiki update를 수행합니다. 한 번에 하나의 mutating 작업만 실행 가능합니다.
        </p>
      </header>

      <Card className="p-4 space-y-3">
        <h2 className="text-lg font-medium">새 작업</h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <CommandButton
            kind="sync"
            label="Sync"
            description="git pull → reindex → ingest → push"
          />
          <CommandButton
            kind="ingest"
            label="Ingest"
            description="새 세션 파싱 + 인덱스"
          />
          <CommandButton
            kind="wiki_update"
            label="Wiki Update"
            description="LLM으로 위키 갱신"
          />
          <CommandButton
            kind="graph_rebuild"
            label="Graph Rebuild"
            description="이미 ingest 된 세션의 시맨틱 그래프 재구축 (since/session/all/retry-failed 옵션)"
          />
        </div>
      </Card>

      <Card className="p-4 space-y-3">
        <h2 className="text-lg font-medium flex items-center gap-2">
          <Activity className="size-4" />
          현재 활성 작업 ({activeJobs.length})
        </h2>
        {activeJobs.length ? (
          <div className="space-y-2">
            {activeJobs.map((j) => (
              <JobItem key={j.id} job={j} />
            ))}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">활성 작업 없음</div>
        )}
      </Card>

      <Card className="p-4 space-y-3">
        <h2 className="text-lg font-medium flex items-center gap-2">
          <History className="size-4" />
          최근 작업 ({recentJobs.length})
        </h2>
        {recentJobs.length ? (
          <div className="space-y-2">
            {recentJobs.map((j) => (
              <JobItem key={j.id} job={j} />
            ))}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">최근 작업 없음</div>
        )}
      </Card>
    </div>
  );
}
