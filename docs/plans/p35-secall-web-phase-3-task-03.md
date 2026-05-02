---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p35-secall-web-phase-3
task_id: 03
parallel_group: A
depends_on: []
---

# Task 03 — Code-split (라우트 + vendor)

## Changed files

수정:
- `web/src/routes/router.tsx:1-30` — 5개 라우트 컴포넌트 (`SessionsRoute`, `SessionDetailRoute`, `DailyRoute`, `WikiRoute`, `CommandsRoute`)를 `React.lazy()`로 dynamic import. `Layout`은 모든 라우트의 부모이므로 eager 유지. `<Suspense fallback={<RouteFallback/>}>` 으로 감쌈.
- `web/vite.config.ts:16-19` — `build.rollupOptions.output.manualChunks` 추가 — vendor chunks 분리.

신규:
- `web/src/components/RouteFallback.tsx` — 라우트 lazy load 중 표시할 작은 스피너.

## Change description

### 1. `RouteFallback` (신규 — UX 일관성)

```tsx
import { Loader2 } from "lucide-react";

/**
 * React.lazy로 코드 분할된 라우트 컴포넌트의 chunk fetch 동안 표시.
 * 다른 로딩 표시(`SessionDetailRoute`의 "세션 불러오는 중…" 등)와 구분되도록 단순한 형태.
 */
export function RouteFallback() {
  return (
    <div className="p-8 flex items-center justify-center text-muted-foreground text-sm">
      <Loader2 className="size-4 animate-spin mr-2" /> 화면 로드 중…
    </div>
  );
}
```

### 2. `router.tsx` 전면 교체

```tsx
import { lazy, Suspense } from "react";
import { createBrowserRouter, Navigate } from "react-router";
import Layout from "./Layout";
import { RouteFallback } from "@/components/RouteFallback";
import { SessionEmptyState } from "./SessionsRoute"; // 작은 컴포넌트 — eager OK

// 라우트 단위 lazy chunks
const SessionsRoute = lazy(() => import("./SessionsRoute"));
const SessionDetailRoute = lazy(() => import("./SessionDetailRoute"));
const DailyRoute = lazy(() => import("./DailyRoute"));
const WikiRoute = lazy(() => import("./WikiRoute"));
const CommandsRoute = lazy(() => import("./CommandsRoute"));

const lazyEl = (Comp: React.LazyExoticComponent<React.ComponentType>) => (
  <Suspense fallback={<RouteFallback />}>
    <Comp />
  </Suspense>
);

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: <Navigate to="/sessions" replace /> },
      {
        path: "sessions",
        element: lazyEl(SessionsRoute),
        children: [
          { index: true, element: <SessionEmptyState /> },
          { path: ":id", element: lazyEl(SessionDetailRoute) },
        ],
      },
      { path: "daily", element: lazyEl(DailyRoute) },
      { path: "daily/:date", element: lazyEl(DailyRoute) },
      { path: "wiki", element: lazyEl(WikiRoute) },
      { path: "wiki/:project", element: lazyEl(WikiRoute) },
      { path: "commands", element: lazyEl(CommandsRoute) },
    ],
  },
]);
```

`SessionEmptyState`는 named export로 그대로 가져옴 (sessions 라우트 index가 lazyless로 즉시 렌더). `SessionsRoute`의 default export는 lazy로 가져오고 `SessionEmptyState`만 eager 가져오는 패턴. **검증 필요**: SessionsRoute.tsx가 `export default function ... + export function SessionEmptyState`로 두 개를 동시 export하는 형태인지 — `web/src/routes/router.tsx:3` 기존 import 형태가 `import SessionsRoute, { SessionEmptyState } from "./SessionsRoute"`이므로 이미 그런 구조. 다만 lazy 분리 시 SessionEmptyState도 같은 chunk에 들어가는지 확인. 만약 vite가 split 시 named export가 같이 묶이면 OK. 아니면 SessionEmptyState를 별도 파일 `web/src/components/SessionEmptyState.tsx`로 분리 (본 task 범위 내).

**실행 방안**: 먼저 기본 lazy split 시도 → `pnpm build` 결과 확인. SessionEmptyState가 main chunk에 있으면 OK, 별도 chunk면 SessionsRoute lazy로 같이 묶기 위해 SessionEmptyState를 별도 모듈로 분리.

### 3. Vite manualChunks (vite.config.ts)

```ts
import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8080",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks: {
          "vendor-react": ["react", "react-dom", "react-router"],
          "vendor-query": ["@tanstack/react-query"],
          "vendor-radix": [
            "@radix-ui/react-dialog",
            "@radix-ui/react-scroll-area",
            "@radix-ui/react-select",
            "@radix-ui/react-separator",
            "@radix-ui/react-slot",
          ],
          "vendor-viz": ["@dagrejs/dagre", "lucide-react"],
        },
      },
    },
    // 분리 후 각 chunk가 500 kB 이하 — warning limit 유지 (분할이 무너지면 경고)
    chunkSizeWarningLimit: 500,
  },
});
```

vendor 그룹은 `web/package.json`의 dependencies 기준. 실제 dep 이름은 `pnpm list --depth=0` 또는 `package.json` 직접 확인:
- React 그룹: react, react-dom, react-router
- TanStack Query: @tanstack/react-query
- shadcn 기반 Radix: @radix-ui/* (실제 설치된 것만 — 빌드 시 일치 안하면 무시됨)
- 시각화: @dagrejs/dagre, lucide-react

설치되지 않은 모듈명은 Vite가 무시 → 안전. 실제 검증은 빌드 결과 chunk 파일명으로.

### 4. `App.tsx` / `main.tsx` 영향

`main.tsx`가 `RouterProvider` 사용하는지 확인. `<RouterProvider router={router} />` 하나면 본 task 변경 영향 없음.

## Dependencies

- 외부 npm: 없음 (모두 기존 의존성 활용)
- 내부 task: 없음 (Task 00/01/02와 독립)

## Verification

```bash
cd /Users/d9ng/privateProject/seCall/web && pnpm typecheck
cd /Users/d9ng/privateProject/seCall/web && pnpm build 2>&1 | tee /tmp/p35_build.log

# chunk 크기 분리 검증 — 500 kB 초과 chunk 없으면 OK
grep -E "dist/assets/.*\.js" /tmp/p35_build.log
# 기대 출력: vendor-react, vendor-query, vendor-radix, vendor-viz, index 등 여러 chunk
# 단일 978 kB → 다중 (각 < 500 kB)

# warning 사라졌는지 확인 (있으면 manualChunks 보완 필요)
grep -E "chunks are larger than" /tmp/p35_build.log && echo "WARNING REMAINS" || echo "OK"

# 라이브 (서버 실행 필요):
# secall serve --bind 127.0.0.1:8080 &
# 브라우저 /sessions → DevTools Network → 초기 진입 시 sessions chunk만 로드
# /commands 클릭 → commands chunk lazy 로드 (<200 kB)
```

## Risks

- **lazy + react-router v7 호환**: react-router v7는 `<Suspense>` 자동 처리 안 함 → 수동 wrap 필요. 본 task의 `lazyEl` 헬퍼가 처리. OR react-router v7의 `lazy` route option 사용도 가능 (`{ path: ..., lazy: () => import(...) }` 형식). 기존 `createBrowserRouter` 그대로 쓰면 Suspense wrapper 방식이 더 단순.
- **SessionEmptyState 위치**: SessionsRoute가 lazy인데 SessionEmptyState는 eager로 같은 파일에서 가져오면, vite가 SessionsRoute chunk를 항상 같이 로드 → lazy 효과 없음. 검증 후 필요하면 SessionEmptyState를 별도 컴포넌트 파일로 분리.
- **vendor manualChunks 매칭 실패**: 패키지명이 정확히 일치 안 하면 그 모듈은 main chunk로 떨어짐 → 분리 효과 부분적. 빌드 결과 chunk 파일명으로 검증.
- **chunkSizeWarningLimit 500**: 분할이 효과적이면 경고 없음. 효과적이지 않으면 경고로 알림 → 본 task 내에서 manualChunks 보완.
- **Suspense fallback 깜빡임**: 라우트 진입 시 짧은 스피너 노출. UX 약간 영향 있지만 단축키/캐시로 첫 진입 후 즉시 로드.
- **prefetch**: 라우트 hover 시 prefetch는 본 task 외 (Phase 4+). 단순 lazy로 시작.

## Scope boundary

수정 금지:
- `crates/` 전체 — 백엔드와 무관
- `web/src/lib/*` — Task 01 영역의 일부와 무관
- `web/src/components/SessionList.tsx` — Task 02 영역
- `web/src/components/SessionEmptyState 분리 시 신규 파일은 본 task 내에서 OK
- `web/src/hooks/*` — 무관
- `web/src/routes/{Layout,SessionsRoute,SessionDetailRoute,DailyRoute,WikiRoute,CommandsRoute,GraphRoute}.tsx` — lazy 대상이지만 컴포넌트 자체 코드는 무수정 (router.tsx의 import 형태만 변경)
- `web/package.json` — 새 dep 추가 안 함
- `.github/`, `README*` — Task 04 영역
