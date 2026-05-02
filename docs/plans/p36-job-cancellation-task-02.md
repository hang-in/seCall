---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p36-job-cancellation
task_id: 02
parallel_group: A
depends_on: []
---

# Task 02 — web UI cancel 버튼 + useCancelJob mutation

## Changed files

수정:
- `web/src/hooks/useJob.ts:65` — `useStartJob` 다음에 `useCancelJob()` mutation hook 신규 추가. `api.cancelJob(id)` 래핑, onSuccess 시 `["jobs"]` + `["job", id]` 캐시 invalidate.
- `web/src/components/JobItem.tsx:19` — status 가 `"started"` 또는 `"running"` 일 때 우측에 "취소" 버튼 마운트. 클릭 시 `window.confirm` 확인 후 `useCancelJob` mutation 호출. mutation pending 상태에서는 버튼 비활성 + 로더 아이콘.
- `web/src/components/JobBanner.tsx:13` — 활성 job(첫 번째) 정보 옆에 동일한 cancel 버튼 마운트. 다중 활성은 단일 큐 정책상 거의 발생 안 함 (P33).

신규: 없음 (`api.cancelJob` 은 P33 Task 03 에서 이미 정의됨)

## Change description

### 핵심 설계

cancel 동작 흐름:
1. 사용자가 JobBanner 또는 JobItem(running) 의 "취소" 버튼 클릭
2. `window.confirm("이 {kind} 작업을 취소하시겠습니까?")` 로 confirm
3. `useCancelJob` mutation 발화 → POST `/api/jobs/{id}/cancel`
4. onSuccess: 캐시 invalidate → ActiveJobs 목록 / 해당 job 상세 refetch
5. SSE 가 살아 있으면 곧 `Failed { error: "cancelled by user" }` 이벤트 수신 → JobItem 의 reducer 가 status=failed/interrupted 로 갱신 (Task 00 백엔드가 status=interrupted 로 마킹)
6. 캐시 invalidate 가 SSE 보다 먼저 결과 노출하면 polling refetch 로 status=interrupted 표시

### `useCancelJob` 계약

- 시그니처: `useCancelJob() -> UseMutationResult`
- mutationFn: `(jobId: string) => api.cancelJob(jobId)`
- onSuccess 시 invalidate 키: `["jobs"]` (active/recent 양쪽), `["job", jobId]` (해당 상세)
- onError: 콘솔 로그(개발 단계). sonner toast 통합은 별도 task.

### UI 표기 규칙

| 상태 | 표시 |
|---|---|
| idle | "취소" 버튼 (X 아이콘) |
| pending | "취소 중…" + 회전 로더, 버튼 disabled |
| 성공 후 | 버튼 자동으로 사라짐 (status 가 active 아니게 되면 조건 false) |
| 실패 (404 등) | 콘솔 에러, 버튼은 다시 활성 |

### confirm 다이얼로그 결정

`window.confirm` 사용 — 단순성 + 의존성 없음. 향후 shadcn `AlertDialog` 로 마이그레이션은 별도 phase.

### 백엔드 미완성 graceful 동작

Task 00 미완료 상태로 본 task 만 머지되면 `cancelJob` 호출이 501 반환 → mutation onError → 콘솔 에러 + UI 는 그대로. 서비스 차단 없음.

## Dependencies

- 외부 npm: 없음 (lucide-react, @tanstack/react-query 이미 사용 중)
- 내부 task: 없음 (Task 00 미완료여도 mutation 정의 + 호출은 가능, onError 로 graceful)

## Verification

```bash
pnpm --dir /Users/d9ng/privateProject/seCall/web typecheck
pnpm --dir /Users/d9ng/privateProject/seCall/web build

# 라이브 (Task 00 + 01 + 02 모두 완료 후, 서버 + active job 필요):
# secall serve & 후 /commands 에서 sync 시작 → 1초 후 banner/job item 의 취소 버튼 클릭
# → confirm → 5초 이내 status 가 interrupted 표시
```

## Risks

- **`window.confirm` UX**: 디자인 통일성 떨어짐. 향후 shadcn AlertDialog 권장.
- **mutation race**: 사용자가 취소 클릭 직후 SSE 가 이미 Done 이벤트 받으면 mutation 은 idempotent 200 (Task 00 처리). UI 는 status 자동 갱신.
- **다중 active job**: JobBanner 는 첫 번째만 취소. 나머지는 JobItem 별 개별 취소. 단일 큐 정책상 다중 활성은 거의 없음.
- **toast 부재**: 실패 안내가 console only — 프로덕션 UX 부족. sonner 통합은 별도 task.
- **버튼 위치**: JobItem 의 기존 레이아웃(progress bar, status badge 등) 과 정렬 디벨로퍼가 결정. ml-auto 또는 flex 정렬로 우측 끝 권장.

## Scope boundary

수정 금지:
- `crates/` 전체 — Task 00 / 01 영역
- `web/src/lib/api.ts` (cancelJob 이미 정의, 시그니처 변경 없음)
- `web/src/hooks/useJob.ts` 의 기존 hook 본문 (`useActiveJobs`, `useStartJob` 등) — 추가만
- `web/src/components/JobOptionsDialog.tsx` — 시작 다이얼로그, 본 task 와 무관
- `web/src/hooks/useJobLifecycle.ts`, `web/src/hooks/useJobStream.ts` — 무관
- `web/src/routes/`, `web/src/lib/types.ts`, `web/src/lib/store.ts` — 무관
- `README*`, `.github/` — Task 03 영역
