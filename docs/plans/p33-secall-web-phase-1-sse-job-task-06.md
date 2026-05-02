---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 06
parallel_group: F
depends_on: [05]
---

# Task 06 — Web UI: 글로벌 진행 배너 + SSE 재연결

## Changed files

수정:
- `web/src/routes/Layout.tsx` — `<JobBanner />` 마운트 (main 영역 상단)
- `web/src/lib/store.ts` — Zustand store에 (선택) 활성 job 캐시 — 또는 TanStack Query useActiveJobs로 충분

신규:
- `web/src/components/JobBanner.tsx` — 글로벌 상단 배너 (활성 job 있을 때만 표시)
- `web/src/components/JobToastListener.tsx` — 활성 job 완료/실패 시 toast 발행 (옵션)
- `web/src/hooks/useJobLifecycle.ts` — job 상태 변화 감지 → toast 발행

## Change description

### 1. `JobBanner.tsx`

```tsx
import { useNavigate } from "react-router";
import { Activity, ChevronRight, Loader2 } from "lucide-react";
import { useActiveJobs } from "@/hooks/useJob";

export function JobBanner() {
  const { data } = useActiveJobs();
  const navigate = useNavigate();

  if (!data?.jobs.length) return null;

  return (
    <div className="border-b border-border bg-accent/40 px-4 py-2 flex items-center gap-3 text-sm">
      <Loader2 className="size-4 animate-spin text-primary" />
      <div className="flex-1 min-w-0">
        <span className="font-medium">{data.jobs.length}개 작업 실행 중</span>
        {data.jobs[0].current_phase && (
          <span className="text-muted-foreground ml-2">
            · {data.jobs[0].kind} / {data.jobs[0].current_phase}
            {typeof data.jobs[0].progress === "number" && (
              <span> ({Math.round(data.jobs[0].progress * 100)}%)</span>
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
```

여러 job 동시 실행은 단일 큐 정책상 1개만이지만, 미래 확장 위해 N개 표시 패턴 유지.

### 2. `useJobLifecycle.ts` (자동 toast)

활성 job 목록에서 새로 완료/실패한 job을 감지하여 toast 발행:
```ts
import { useEffect, useRef } from "react";
import { toast } from "sonner";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { JobState } from "@/lib/types";

/**
 * 모든 페이지에서 현재 활성 job들을 추적하다가, 상태가 active → completed/failed로 바뀌면 toast 발행.
 * 폴링 간격은 useActiveJobs와 동일 (5초).
 */
export function useJobLifecycle() {
  const previous = useRef<Map<string, string>>(new Map());
  const qc = useQueryClient();

  // 최근 jobs 폴링 (recent + active 모두 포함하기 위해 별도 쿼리)
  const { data } = useQuery({
    queryKey: ["jobs", "lifecycle"],
    queryFn: () => api.listRecentJobs(),
    refetchInterval: 5000,
  });

  useEffect(() => {
    if (!data?.jobs) return;
    for (const job of data.jobs) {
      const prevStatus = previous.current.get(job.id);
      if (prevStatus && prevStatus !== job.status) {
        // 상태 변화 감지
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
          toast.warning(`${job.kind} 중단됨 (서버 재시작)`, {
            description: "이전에 실행 중이던 작업이 서버 재시작으로 중단되었습니다.",
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
  // sync 결과: { pulled, reindexed, ingested, pushed }
  if (job.kind === "sync") {
    return `pulled=${r.pulled ?? "-"} / reindexed=${r.reindexed ?? 0} / ingested=${r.ingested ?? 0}`;
  }
  if (job.kind === "ingest") {
    return `${r.new_sessions ?? 0}개 신규 세션`;
  }
  if (job.kind === "wiki_update") {
    return `${r.pages ?? 0}개 위키 페이지 갱신`;
  }
  return undefined;
}
```

### 3. Layout 통합

`web/src/routes/Layout.tsx`:
```tsx
import { JobBanner } from "@/components/JobBanner";
import { useJobLifecycle } from "@/hooks/useJobLifecycle";

export default function Layout() {
  useJobLifecycle();  // 전역 toast listener
  // ... 기존 코드
  return (
    <div className="flex h-screen bg-background text-foreground">
      <aside>...</aside>
      <main className="flex-1 overflow-hidden flex flex-col">
        <JobBanner />
        <div className="flex-1 overflow-hidden">
          <Outlet />
        </div>
      </main>
      <GraphOverlay />
    </div>
  );
}
```

> `useJobLifecycle`은 매 5초 polling — SSE가 아니라 polling 기반. SSE는 단일 job stream에 사용하고, 글로벌 lifecycle 감지는 polling으로 단순하게.

### 4. SSE 재연결 처리 (Task 06의 useJobStream 보강)

`web/src/hooks/useJobStream.ts`를 다음과 같이 보강:
```ts
export function useJobStream(
  id: string | undefined,
  onEvent: (e: ProgressEvent) => void,
  enabled: boolean = true,
) {
  useEffect(() => {
    if (!id || !enabled) return;
    let es: EventSource | null = null;
    let reconnectTimer: number | null = null;

    const connect = () => {
      es = new EventSource(`/api/jobs/${encodeURIComponent(id)}/stream`);
      es.onmessage = (m) => {
        try {
          onEvent(JSON.parse(m.data) as ProgressEvent);
        } catch {}
      };
      es.onerror = () => {
        es?.close();
        // 5초 후 재연결 (job이 active일 때만 — 외부에서 enabled로 제어)
        reconnectTimer = window.setTimeout(connect, 5000);
      };
    };

    connect();
    return () => {
      if (reconnectTimer) clearTimeout(reconnectTimer);
      es?.close();
    };
  }, [id, enabled, onEvent]);
}
```

Task 06의 JobItem이 `enabled = job.status === "running" || job.status === "started"` 전달하면 완료 후엔 재연결 안 함.

### 5. 단위 테스트 / 라이브

본 task는 UX 통합 위주라 단위 테스트는 적음. 라이브 검증으로 충분:
- 탭 닫기 → 다시 열기: 활성 job 자동 복원, 진행 표시 재개
- 다른 페이지로 이동: 상단 배너 유지
- Job 완료: toast 발행 + 배너 사라짐
- 서버 재시작 후 페이지 reload: interrupted toast 표시

## Dependencies

- Task 06 완료 (CommandsRoute, useJob, useJobStream, useActiveJobs)
- 외부 npm 추가 없음

## Verification

```bash
# 1. typecheck + build
cd web && pnpm typecheck && pnpm build

# 2. cargo check (영향 없음)
cd .. && cargo check --all-targets

# 3. 라이브 검증 (수동)
./target/release/secall serve --port 18095 &
SP=$!
sleep 3
cd web && pnpm dev &
DEV=$!
sleep 3

# 브라우저:
# - http://127.0.0.1:5173/commands → Sync (dry_run + local_only) 시작
# - 다른 메뉴(Sessions/Daily/Wiki)로 이동 → 상단에 배너 유지
# - 탭 닫고 다시 열기 → 배너 자동 복원
# - 완료 시 toast 알림

kill $DEV $SP 2>/dev/null
wait 2>/dev/null
```

## Risks

- **toast spam**: 이전 상태 추적이 메모리(useRef)라 페이지 reload 시 초기화 → reload 직후 active jobs는 새 알림 안 발행 (의도된 동작)
- **5초 polling**: 5초 단위라 완료 알림이 최대 5초 늦을 수 있음. UX 허용 범위
- **재연결 무한 루프**: 서버 다운 시 5초마다 재시도. enabled로 제어하지만 사용자가 활성 페이지에 머물면 계속 시도. 백오프 또는 max_retries 추가 검토 (v1.1)
- **EventSource 브라우저 한계**: 일부 브라우저 (older Safari) 5개 연결 제한. 동시에 여러 JobItem 열면 SSE 다수 — 단일 큐 정책상 active job은 1개라 문제 없음
- **`useJobLifecycle` 글로벌 마운트**: Layout에서 한 번만 호출되어야 함. 여러 page에서 호출되지 않게 주의

## Scope boundary

수정 금지:
- `crates/` 전체
- `web/src/routes/CommandsRoute.tsx` — Task 06
- `web/src/components/{CommandButton,JobItem,JobOptionsDialog}.tsx` — Task 06
- `web/src/hooks/useJob.ts` 본체 — Task 06 (단, useJobStream의 enabled 인자 보강만 본 task에서 허용)
- `.github/workflows/`, `README*` — Task 09
