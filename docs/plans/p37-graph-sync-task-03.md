---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p37-graph-sync
task_id: 03
parallel_group: C
depends_on: [02]
---

# Task 03 — web UI: CommandsRoute 카드 + JobOptionsDialog 옵션 + types/api

## Changed files

수정:
- `web/src/lib/types.ts:76` — `JobKind` 유니온에 `"graph_rebuild"` 추가, `GraphRebuildArgs` / `GraphRebuildOutcome` 타입 추가 (Task 01 의 Rust 구조체와 1:1 매핑).
- `web/src/lib/api.ts:122-138` 인접 — `startGraphRebuild(args)` 메서드 추가 (P33 의 `startSync` / `startIngest` 패턴 그대로). `POST /api/commands/graph-rebuild`.
- `web/src/hooks/useJob.ts:65-91` 인접 — `useStartJob` 의 switch 분기에 `case "graph_rebuild": return api.startGraphRebuild(args as GraphRebuildArgs);` 추가.
- `web/src/components/CommandButton.tsx` — sync/ingest/wiki 카드 옆에 "graph rebuild" 카드 추가 (라벨/설명/버튼). 클릭 시 `JobOptionsDialog` 를 `kind="graph_rebuild"` 로 open.
- `web/src/components/JobItem.tsx` — `JobKind` union 확장으로 인한 exhaustive switch 컴파일 강제 — `renderOutcome` 에 `GraphRebuildOutcome` 분기 추가 (processed/succeeded/failed/skipped/edges_added 표시). 본 task 의 핵심 변경은 아니지만 typecheck 통과 위해 필수.
- `web/src/routes/CommandsRoute.tsx` — 카드 그리드에 4번째 graph_rebuild 카드 노출 위해 `sm:grid-cols-3` → `sm:grid-cols-2` 변경 (2x2 layout).
- `web/src/components/JobOptionsDialog.tsx:88-220` 인접 — sync/ingest/wiki 폼 옆에 `GraphRebuildOptionsForm` 추가:
  - 입력 필드: `since` (date input), `session` (text), `all` (checkbox), `retry_failed` (checkbox)
  - submit 시 `useStartJob("graph_rebuild")` 의 mutate 호출
  - cancel 버튼은 기존 onCancel prop 사용
- `web/src/routes/CommandsRoute.tsx` (필요 시) — 카드 그리드에 graph rebuild 카드가 자동 노출되는지 확인. CommandButton 이 이미 모든 kind 를 매핑하면 변경 없음.

신규: 없음

## Change description

### 1. types.ts 추가 사항

```ts
// 기존 JobKind:
export type JobKind = "sync" | "ingest" | "wiki_update" | "graph_rebuild";

// P37 신규
export interface GraphRebuildArgs {
  since?: string;
  session?: string;
  all?: boolean;
  retry_failed?: boolean;
}

export interface GraphRebuildOutcome {
  processed: number;
  succeeded: number;
  failed: number;
  skipped: number;
  edges_added: number;
}
```

### 2. api.ts 메서드

P33 의 `startSync` / `startIngest` 그대로 모방:
- 시그니처: `startGraphRebuild: (args: GraphRebuildArgs) => Promise<JobStartResponse>`
- 본문: `jfetch<JobStartResponse>("/api/commands/graph-rebuild", { method: "POST", body: JSON.stringify(args) })`

### 3. useJob.ts switch 확장

기존 switch (line 69-76) 에 한 case 추가:
```ts
case "graph_rebuild":
  return api.startGraphRebuild(args as GraphRebuildArgs);
```

`JobArgs` 타입 union 에 `GraphRebuildArgs` 도 포함되도록 확장 (필요 시 union type 정의 수정).

### 4. CommandButton.tsx 카드 추가

기존 sync/ingest/wiki 카드 정의를 보고 같은 패턴으로 graph_rebuild 카드 추가:
- 제목: "Graph Rebuild"
- 설명: "이미 ingest 된 세션의 시맨틱 그래프 재구축. since/session/all/retry-failed 옵션 지원."
- onClick → setSelectedKind("graph_rebuild")

### 5. JobOptionsDialog.tsx 폼 추가

기존 SyncOptionsForm / IngestOptionsForm / WikiOptionsForm 패턴 따라:
```tsx
function GraphRebuildOptionsForm({ onSubmit, onCancel }: { onSubmit: (args: GraphRebuildArgs) => void; onCancel: () => void }) {
  const [since, setSince] = useState("");
  const [session, setSession] = useState("");
  const [all, setAll] = useState(false);
  const [retryFailed, setRetryFailed] = useState(false);
  // ...form fields...
}
```

dialog 의 kind switch 에 분기 추가:
```tsx
case "graph_rebuild":
  return <GraphRebuildOptionsForm onSubmit={...} onCancel={...} />;
```

### 6. UX 우선순위 안내

폼 안에 짧은 안내 문구 — "session > all > retry-failed > since 우선순위" — 사용자가 동시에 여러 필드 입력 시 어떤 게 적용되는지 명시 (Task 01 의 SQL 우선순위 그대로).

## Dependencies

- 외부 npm: 없음
- 내부 task: **Task 02 완료 필수** — `/api/commands/graph-rebuild` 엔드포인트가 있어야 mutation 동작. 미완료 시 404 → onError graceful (사용자에게 표시는 콘솔).

## Verification

```bash
pnpm --dir /Users/d9ng/privateProject/seCall/web typecheck
pnpm --dir /Users/d9ng/privateProject/seCall/web build

# 라이브 (서버 + Task 00-02 완료 필요):
# secall serve & 후 /commands → "Graph Rebuild" 카드 클릭
# → 옵션 다이얼로그에서 retry_failed 체크 후 시작
# → JobBanner 진행률 + cancel 버튼 (P36) 동작 확인
```

## Risks

- **JobKind union 확장의 타입 영향**: 기존 switch / Map 호출에 모두 분기 추가 필요. tsc 가 미분기 케이스 잡아줌 — typecheck 통과 = 모든 분기 처리.
- **JobArgs union**: useStartJob mutationFn 시그니처가 union 받음 → as 캐스팅 필요. P33 의 기존 패턴 그대로.
- **카드 그리드 layout**: 4번째 카드 추가 시 grid 가 반응형으로 재배치. CSS 검증 시각적으로 확인.
- **다이얼로그 폼 검증**: since 가 잘못된 date 형식이면 백엔드가 빈 결과 반환 (Task 01 SQL). 클라이언트 측 validation 은 본 task 외 — placeholder + format hint 정도.
- **session ID 길이**: text input 에 8자 단축 ID 입력 가능 — Task 01 의 `resolve_session_id` 가 prefix 매칭 처리하면 OK. 그렇지 않으면 full ID 강제 안내.

## Scope boundary

수정 금지:
- `crates/` 전체 — Task 00-02 영역
- `web/src/components/{JobBanner,JobToastListener}.tsx` — P33/P36 완료, 본 task 와 무관 (graph_rebuild 도 동일 인터페이스로 자동 노출). `JobItem.tsx` 는 exhaustive switch 강제로 본 task 가 분기 추가 (위 Changed files 참조).
- `web/src/hooks/{useJobLifecycle,useJobStream,useCancelJob}.ts` — 변경 없음
- `web/src/routes/{Sessions,Daily,Wiki,SessionDetail}Route.tsx` — 무관
- `web/src/lib/{store,allTags,tagColor,utils,queryClient,graphStartNode,highlight,graphStyle}.ts` — 무관
- `README*`, `.github/` — Task 04 영역
