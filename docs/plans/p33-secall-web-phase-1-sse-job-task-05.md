---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 05
parallel_group: E
depends_on: [03, 04]
---

# Task 05 — Web UI: Commands 패널 + Job 시작/상태 훅

## Changed files

수정:
- `web/src/routes/Layout.tsx` — 사이드바에 "Commands" 메뉴 추가, 우상단 액션 영역
- `web/src/routes/router.tsx` — `/commands` 라우트 등록
- `web/src/lib/api.ts` — Job 관련 함수 (`startSync`, `startIngest`, `startWikiUpdate`, `getJob`, `listActiveJobs`, `getJobsRecent`)
- `web/src/lib/types.ts` — `JobKind`, `JobStatus`, `JobState`, `ProgressEvent`, `SyncArgs`, `IngestArgs`, `WikiUpdateArgs`

신규:
- `web/src/routes/CommandsRoute.tsx` — 명령 패널 (Sync/Ingest/Wiki Update 버튼 + 옵션 폼 + 활성 job 리스트)
- `web/src/components/CommandButton.tsx` — 명령 시작 버튼 (`useJobMutation` 사용)
- `web/src/components/JobItem.tsx` — 단일 job 카드 (phase / progress / message / 결과)
- `web/src/components/JobOptionsDialog.tsx` — sync/ingest/wiki 옵션 입력 다이얼로그 (RHF + Zod)
- `web/src/hooks/useJob.ts` — `useJob(id)` (단일 폴링/SSE), `useActiveJobs()` (활성 리스트), `useStartJob(kind)` (mutation)
- `web/src/hooks/useJobStream.ts` — EventSource 기반 SSE 구독 + ProgressEvent 처리

## Change description

### 1. API 레이어

`web/src/lib/api.ts`에 추가:
```ts
export const api = {
  // ... 기존
  startSync: (args: SyncArgs) =>
    jfetch<JobStartResponse>("/api/commands/sync", {
      method: "POST",
      body: JSON.stringify(args),
    }),
  startIngest: (args: IngestArgs) =>
    jfetch<JobStartResponse>("/api/commands/ingest", {
      method: "POST",
      body: JSON.stringify(args),
    }),
  startWikiUpdate: (args: WikiUpdateArgs) =>
    jfetch<JobStartResponse>("/api/commands/wiki-update", {
      method: "POST",
      body: JSON.stringify(args),
    }),
  getJob: (id: string) => jfetch<JobState>(`/api/jobs/${encodeURIComponent(id)}`),
  listActiveJobs: () => jfetch<{ jobs: JobState[] }>("/api/jobs?status=active"),
  listRecentJobs: () => jfetch<{ jobs: JobState[] }>("/api/jobs?status=recent"),
  cancelJob: (id: string) =>
    jfetch<unknown>(`/api/jobs/${encodeURIComponent(id)}/cancel`, { method: "POST" }),
};
```

`409 Conflict`는 `jfetch`가 throw — 호출부에서 catch.

### 2. 타입

`web/src/lib/types.ts`에 추가:
```ts
export type JobKind = "sync" | "ingest" | "wiki_update";
export type JobStatus = "started" | "running" | "completed" | "failed" | "interrupted";

export interface JobState {
  id: string;
  kind: JobKind;
  status: JobStatus;
  started_at: string;
  completed_at: string | null;
  current_phase: string | null;
  progress: number | null;
  message: string | null;
  error: string | null;
  result: unknown | null;
  metadata: unknown | null;
}

export interface JobStartResponse {
  job_id: string;
  status: "started";
}

export type ProgressEvent =
  | { type: "phase_start"; phase: string }
  | { type: "message"; text: string }
  | { type: "progress"; ratio: number }
  | { type: "phase_complete"; phase: string; result?: unknown }
  | { type: "done"; result: unknown }
  | { type: "failed"; error: string; partial_result?: unknown };

export interface SyncArgs {
  local_only?: boolean;
  dry_run?: boolean;
  no_wiki?: boolean;
  no_semantic?: boolean;
}

export interface IngestArgs {
  cwd?: string;
  force?: boolean;
  min_turns?: number;
  no_semantic?: boolean;
}

export interface WikiUpdateArgs {
  backend?: string;
  model?: string;
  since?: string;
  session?: string;
  dry_run?: boolean;
  review?: boolean;
}
```

### 3. `useJobStream.ts`

```ts
import { useEffect } from "react";
import type { ProgressEvent } from "@/lib/types";

export function useJobStream(
  id: string | undefined,
  onEvent: (e: ProgressEvent) => void,
) {
  useEffect(() => {
    if (!id) return;
    const es = new EventSource(`/api/jobs/${encodeURIComponent(id)}/stream`);
    es.onmessage = (m) => {
      try {
        const event = JSON.parse(m.data) as ProgressEvent;
        onEvent(event);
      } catch {
        // 무시
      }
    };
    es.onerror = () => {
      es.close();
    };
    return () => es.close();
  }, [id, onEvent]);
}
```

### 4. `useJob.ts`

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { JobKind, JobState, SyncArgs, IngestArgs, WikiUpdateArgs } from "@/lib/types";
import { toast } from "sonner";

export function useJob(id: string | undefined) {
  return useQuery({
    queryKey: ["jobs", id],
    queryFn: () => api.getJob(id!),
    enabled: !!id,
    refetchInterval: (q) => {
      const data = q.state.data as JobState | undefined;
      if (!data) return false;
      return data.status === "completed" || data.status === "failed" || data.status === "interrupted"
        ? false
        : 2000; // SSE가 끊겼을 때 폴링 fallback
    },
  });
}

export function useActiveJobs() {
  return useQuery({
    queryKey: ["jobs", "active"],
    queryFn: api.listActiveJobs,
    refetchInterval: 5000,
  });
}

export function useStartJob(kind: JobKind) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (args: SyncArgs | IngestArgs | WikiUpdateArgs) => {
      switch (kind) {
        case "sync": return api.startSync(args as SyncArgs);
        case "ingest": return api.startIngest(args as IngestArgs);
        case "wiki_update": return api.startWikiUpdate(args as WikiUpdateArgs);
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
```

### 5. `CommandsRoute.tsx`

```tsx
import { Play, Activity } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useActiveJobs } from "@/hooks/useJob";
import { CommandButton } from "@/components/CommandButton";
import { JobItem } from "@/components/JobItem";

export default function CommandsRoute() {
  const { data: active } = useActiveJobs();

  return (
    <div className="p-6 max-w-4xl space-y-6">
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
        <div className="grid grid-cols-3 gap-3">
          <CommandButton kind="sync" label="Sync" description="git pull → reindex → ingest → push" />
          <CommandButton kind="ingest" label="Ingest" description="새 세션 파싱 + 인덱스" />
          <CommandButton kind="wiki_update" label="Wiki Update" description="LLM으로 위키 갱신" />
        </div>
      </Card>

      <Card className="p-4 space-y-3">
        <h2 className="text-lg font-medium flex items-center gap-2">
          <Activity className="size-4" />
          현재 활성 작업 ({active?.jobs.length ?? 0})
        </h2>
        {active?.jobs.length ? (
          <div className="space-y-2">
            {active.jobs.map((j) => <JobItem key={j.id} job={j} />)}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">활성 작업 없음</div>
        )}
      </Card>
    </div>
  );
}
```

### 6. `CommandButton.tsx`

```tsx
import { useState } from "react";
import { Loader2, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useStartJob } from "@/hooks/useJob";
import { JobOptionsDialog } from "./JobOptionsDialog";
import type { JobKind } from "@/lib/types";

interface Props {
  kind: JobKind;
  label: string;
  description: string;
}

export function CommandButton({ kind, label, description }: Props) {
  const [open, setOpen] = useState(false);
  const mutation = useStartJob(kind);

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        disabled={mutation.isPending}
        className="text-left p-3 border border-border rounded hover:bg-accent transition-colors disabled:opacity-50"
      >
        <div className="font-medium flex items-center gap-2">
          {mutation.isPending ? <Loader2 className="size-4 animate-spin" /> : <Play className="size-4" />}
          {label}
        </div>
        <div className="text-xs text-muted-foreground mt-1">{description}</div>
      </button>
      <JobOptionsDialog
        kind={kind}
        open={open}
        onOpenChange={setOpen}
        onSubmit={(args) => {
          mutation.mutate(args);
          setOpen(false);
        }}
      />
    </>
  );
}
```

### 7. `JobItem.tsx`

```tsx
import { useJobStream } from "@/hooks/useJobStream";
import { useState } from "react";
import type { JobState, ProgressEvent } from "@/lib/types";

export function JobItem({ job: initial }: { job: JobState }) {
  const [job, setJob] = useState(initial);

  useJobStream(job.status === "completed" || job.status === "failed" ? undefined : job.id, (e) => {
    setJob((prev) => applyEvent(prev, e));
  });

  return (
    <div className="border border-border rounded p-3 space-y-2">
      <div className="flex items-center justify-between text-sm">
        <span className="font-medium">{job.kind}</span>
        <span className="font-mono text-xs opacity-60">{job.id.slice(0, 8)}</span>
      </div>
      {job.current_phase && (
        <div className="text-xs text-muted-foreground">
          phase: <span className="font-medium">{job.current_phase}</span>
          {typeof job.progress === "number" && <span> · {Math.round(job.progress * 100)}%</span>}
        </div>
      )}
      {job.message && <div className="text-xs whitespace-pre-wrap opacity-80">{job.message}</div>}
      {job.error && <div className="text-xs text-rose-400 whitespace-pre-wrap">{job.error}</div>}
      <div className="flex items-center gap-2 text-xs">
        <StatusBadge status={job.status} />
        <span className="text-muted-foreground tabular-nums">{job.started_at}</span>
      </div>
    </div>
  );
}

function applyEvent(prev: JobState, e: ProgressEvent): JobState {
  switch (e.type) {
    case "phase_start":
      return { ...prev, status: "running", current_phase: e.phase, progress: null };
    case "message":
      return { ...prev, message: e.text };
    case "progress":
      return { ...prev, progress: e.ratio };
    case "phase_complete":
      return { ...prev, message: `${e.phase} 완료` };
    case "done":
      return { ...prev, status: "completed", result: e.result, completed_at: new Date().toISOString() };
    case "failed":
      return { ...prev, status: "failed", error: e.error, completed_at: new Date().toISOString() };
  }
}
```

### 8. `JobOptionsDialog.tsx`

shadcn `Dialog` + RHF + Zod 스키마. kind에 따라 폼 필드 다름:
- sync: local_only, dry_run, no_wiki, no_semantic (4 checkbox)
- ingest: cwd (input), force, min_turns (number), no_semantic
- wiki_update: backend (select: claude/codex/haiku/ollama/lmstudio), since (date), session (input), dry_run, review

기본값은 모두 false/undefined. 빈 값은 API 요청에서 제외.

### 9. Layout 사이드바에 Commands 추가

`web/src/routes/Layout.tsx`의 NAV 배열에:
```tsx
const NAV = [
  { to: "/sessions", icon: Search, label: "Sessions" },
  { to: "/daily", icon: Calendar, label: "Daily" },
  { to: "/wiki", icon: BookOpen, label: "Wiki" },
  { to: "/commands", icon: Play, label: "Commands" },  // 추가
];
```

### 10. 라우터 등록

`web/src/routes/router.tsx`:
```tsx
{ path: "commands", element: <CommandsRoute /> },
```

## Dependencies

- Task 04 완료 (Job REST 엔드포인트)
- Task 05 완료 (WikiRoute 본문 표시)
- 신규 npm 패키지: 없음 (RHF/Zod는 이미 P32에서 추가됨)

## Verification

```bash
# 1. typecheck + build
cd web && pnpm typecheck && pnpm build

# 2. cargo check (영향 없음 확인)
cd .. && cargo check --all-targets

# 3. 라이브 검증
./target/release/secall serve --port 18094 &
SP=$!
sleep 3
cd web && pnpm dev &
DEV=$!
sleep 3

# 브라우저: http://127.0.0.1:5173/commands
# - Sync 버튼 → 옵션 다이얼로그 → dry_run + local_only 체크 → 시작
# - 활성 작업 목록에 새 job 표시, phase 진행 + message 보임
# - 완료 시 toast 알림

# 정리
kill $DEV $SP 2>/dev/null
wait 2>/dev/null
```

## Risks

- **EventSource CORS**: same-origin (Vite proxy + axum)이라 문제 없지만, 프록시 경유 시 SSE keep-alive 헤더 처리 검토 (Vite는 보통 OK)
- **EventSource auto-reconnect**: 기본 동작 — 1초 후 재연결. 서버 종료 시 무한 재시도 → onerror에서 `es.close()`로 막음 (위 코드)
- **폴링 vs SSE 중복**: SSE 활성 시 useJob의 refetchInterval과 중복. SSE가 마지막 이벤트로 status를 업데이트하므로 polling이 곧 멈춤 — 의도된 fallback
- **toast 누락**: 작업 완료 toast는 JobItem 내부 또는 별도 listener에서 발행. 본 task는 시작/실패만 toast, 완료는 JobItem 시각 표시 (Task 07에서 글로벌 toast 통합 가능)
- **dialog 폼 검증**: Zod 스키마로 클라이언트 검증. 잘못된 값은 서버에서 한 번 더 검증
- **bundle 크기**: 약 30~50KB 증가 예상 (RHF + Dialog 이미 있음)

## Scope boundary

수정 금지:
- `crates/` 전체 — 백엔드 변경 금지 (Read만 응답 구조 확인용)
- `web/src/routes/{Sessions,SessionDetail,Daily,Wiki}Route.tsx` — Task 05만 Wiki 손댐, 본 task는 Layout/router/Commands만
- `web/src/components/{Session*,TagEditor,FavoriteButton,DateNavigator,Graph*}.tsx` — Task 06 영역 외
- `.github/workflows/`, `README*` — Task 09
