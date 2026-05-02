---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 04
parallel_group: B
depends_on: [00]
---

# Task 04 — React 프론트 핵심 셋업

## Changed files

수정:
- `web/package.json` — 본격 의존성 추가 (Tailwind, shadcn/ui, Zustand, TanStack Query, RHF, Zod, Lucide)
- `web/src/main.tsx` — Provider 트리 구성 (QueryClientProvider, RouterProvider)
- `web/src/App.tsx` — 라우팅 outlet으로 전환 (Layout 컴포넌트 placeholder)
- `web/index.html` — 폰트 로드 (Pretendard, Geist Sans), 다크 모드 기본 클래스
- `web/vite.config.ts` — `@/` alias 추가

신규:
- `web/tailwind.config.ts` — Tailwind 설정 (다크 모드, 폰트 패밀리)
- `web/postcss.config.js` — PostCSS + Tailwind
- `web/src/index.css` — Tailwind base/components/utilities + CSS 변수 (다크 테마)
- `web/src/lib/api.ts` — fetch 래퍼 + 엔드포인트 함수 (recall, getSession, listSessions, listProjects, listAgents, daily, graph, setTags, setFavorite, status, wikiSearch)
- `web/src/lib/queryClient.ts` — TanStack Query 클라이언트 인스턴스
- `web/src/lib/store.ts` — Zustand 스토어 (UI 상태: 사이드바, 그래프 오버레이 등)
- `web/src/lib/tagColor.ts` — 태그명 → 색상 결정론적 해시
- `web/src/lib/utils.ts` — shadcn/ui 표준 (`cn` 함수)
- `web/src/routes/router.tsx` — React Router v7 라우트 정의
- `web/src/routes/Layout.tsx` — 사이드바 + 메인 영역 (Outlet)
- `web/src/routes/SessionsRoute.tsx` — 세션 리스트 placeholder (Task 06에서 본격 구현)
- `web/src/routes/SessionDetailRoute.tsx` — 세션 상세 placeholder
- `web/src/routes/DailyRoute.tsx` — 일일 일기 placeholder (Task 07)
- `web/src/routes/WikiRoute.tsx` — 위키 placeholder (Task 07)
- `web/src/routes/GraphRoute.tsx` — 그래프 placeholder (Task 08)
- `web/src/components/ui/` — shadcn 기본 컴포넌트 (Button, Input, Sheet, Badge, Toast, Dialog, Card, Separator, ScrollArea)
- `web/components.json` — shadcn/ui CLI 설정

## Change description

### 1. 의존성 추가

`web/package.json`:
```json
{
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router": "^7.0.0",
    "@tanstack/react-query": "^5.59.0",
    "zustand": "^5.0.0",
    "react-hook-form": "^7.53.0",
    "@hookform/resolvers": "^3.9.0",
    "zod": "^3.23.8",
    "clsx": "^2.1.1",
    "tailwind-merge": "^2.5.4",
    "class-variance-authority": "^0.7.0",
    "lucide-react": "^0.454.0",
    "react-markdown": "^9.0.1",
    "remark-gfm": "^4.0.0",
    "sonner": "^1.7.0",
    "@radix-ui/react-slot": "^1.1.0",
    "@radix-ui/react-dialog": "^1.1.2",
    "@radix-ui/react-toast": "^1.2.2",
    "@radix-ui/react-separator": "^1.1.0",
    "@radix-ui/react-scroll-area": "^1.2.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.3",
    "typescript": "^5.6.3",
    "vite": "^5.4.10",
    "tailwindcss": "^3.4.14",
    "postcss": "^8.4.47",
    "autoprefixer": "^10.4.20"
  }
}
```

> shadcn/ui는 CLI로 컴포넌트를 복사하는 방식 — 별도 npm 패키지 아님. radix-ui 프리미티브만 의존.

### 2. Tailwind + 다크 모드

`web/tailwind.config.ts`:
```ts
import type { Config } from "tailwindcss";

export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["Pretendard Variable", "Geist Sans", "system-ui", "sans-serif"],
        mono: ["Geist Mono", "ui-monospace", "monospace"],
      },
      colors: {
        // shadcn/ui 표준 CSS 변수 매핑
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        card: "hsl(var(--card))",
        "card-foreground": "hsl(var(--card-foreground))",
        primary: "hsl(var(--primary))",
        "primary-foreground": "hsl(var(--primary-foreground))",
        secondary: "hsl(var(--secondary))",
        "secondary-foreground": "hsl(var(--secondary-foreground))",
        muted: "hsl(var(--muted))",
        "muted-foreground": "hsl(var(--muted-foreground))",
        accent: "hsl(var(--accent))",
        "accent-foreground": "hsl(var(--accent-foreground))",
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
      },
      borderRadius: {
        lg: "8px",
        md: "6px",
        sm: "4px",
      },
    },
  },
  plugins: [],
} satisfies Config;
```

`web/src/index.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    /* 다크 모드 기본 — Linear/Vercel 톤 */
    --background: 0 0% 7%;
    --foreground: 0 0% 96%;
    --card: 0 0% 10%;
    --card-foreground: 0 0% 96%;
    --primary: 263 70% 65%;       /* violet 액센트 */
    --primary-foreground: 0 0% 100%;
    --secondary: 0 0% 14%;
    --secondary-foreground: 0 0% 96%;
    --muted: 0 0% 14%;
    --muted-foreground: 0 0% 64%;
    --accent: 0 0% 18%;
    --accent-foreground: 0 0% 96%;
    --border: 0 0% 18%;
    --input: 0 0% 18%;
    --ring: 263 70% 65%;
  }

  body {
    @apply bg-background text-foreground;
    font-feature-settings: "ss01", "cv01";
  }
}
```

### 3. 폰트 로드

`web/index.html` `<head>`에 추가:
```html
<link rel="preconnect" href="https://cdn.jsdelivr.net" />
<link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/orioncactus/pretendard/dist/web/variable/pretendardvariable-dynamic-subset.css" />
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/geist@1.3.1/dist/fonts/geist-sans/style.css" />
```

`<html>` 또는 `<body>`에 `class="dark"` 명시 (다크 모드 기본).

### 4. `@/` alias

`web/vite.config.ts`:
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
    proxy: { "/api": "http://127.0.0.1:8080" },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
```

`web/tsconfig.json`에:
```json
{
  "compilerOptions": {
    ...,
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  }
}
```

### 5. shadcn/ui 초기 컴포넌트

`web/components.json` (shadcn CLI 설정):
```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "tailwind.config.ts",
    "css": "src/index.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui"
  }
}
```

`pnpm dlx shadcn@latest init` + `pnpm dlx shadcn@latest add button input sheet badge dialog card separator scroll-area sonner` 실행으로 컴포넌트 생성 (수동 add 명령은 task 수행 시 실행).

### 6. API 레이어

`web/src/lib/api.ts`:
```ts
const BASE = "";  // dev/prod 모두 same-origin

async function jfetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + path, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }
  return res.json();
}

export interface SessionListItem {
  id: string;
  agent: string;
  project: string | null;
  model: string | null;
  date: string;
  start_time: string;
  turn_count: number;
  summary: string | null;
  tags: string[];
  is_favorite: boolean;
  session_type: string;
  vault_path: string | null;
}

export interface SessionListPage {
  items: SessionListItem[];
  total: number;
  page: number;
  page_size: number;
}

export const api = {
  recall: (q: { query: string; mode?: string; limit?: number; project?: string; agent?: string }) =>
    jfetch<unknown>("/api/recall", { method: "POST", body: JSON.stringify(q) }),

  getSession: (session_id: string, full = false) =>
    jfetch<unknown>("/api/get", { method: "POST", body: JSON.stringify({ session_id, full }) }),

  listSessions: (params: Partial<{
    page: number; page_size: number; project: string; agent: string;
    date_from: string; date_to: string; tag: string; favorite: boolean; q: string;
  }>) => {
    const qs = new URLSearchParams();
    Object.entries(params).forEach(([k, v]) => v !== undefined && qs.set(k, String(v)));
    return jfetch<SessionListPage>(`/api/sessions?${qs}`);
  },

  listProjects: () => jfetch<{ projects: string[] }>("/api/projects"),
  listAgents: () => jfetch<{ agents: string[] }>("/api/agents"),

  setTags: (id: string, tags: string[]) =>
    jfetch<{ session_id: string; tags: string[] }>(
      `/api/sessions/${encodeURIComponent(id)}/tags`,
      { method: "PATCH", body: JSON.stringify({ tags }) },
    ),

  setFavorite: (id: string, favorite: boolean) =>
    jfetch<{ session_id: string; favorite: boolean }>(
      `/api/sessions/${encodeURIComponent(id)}/favorite`,
      { method: "PATCH", body: JSON.stringify({ favorite }) },
    ),

  status: () => jfetch<unknown>("/api/status"),
  daily: (date?: string) =>
    jfetch<unknown>("/api/daily", { method: "POST", body: JSON.stringify({ date }) }),
  graph: (q: { node_id: string; depth?: number; relation?: string }) =>
    jfetch<unknown>("/api/graph", { method: "POST", body: JSON.stringify(q) }),
  wikiSearch: (q: { query: string; limit?: number }) =>
    jfetch<unknown>("/api/wiki", { method: "POST", body: JSON.stringify(q) }),
};
```

### 7. Zustand UI 스토어

`web/src/lib/store.ts`:
```ts
import { create } from "zustand";

interface UiState {
  sidebarOpen: boolean;
  graphOverlayOpen: boolean;
  selectedSessionId: string | null;
  toggleSidebar: () => void;
  toggleGraphOverlay: () => void;
  setSelectedSession: (id: string | null) => void;
}

export const useUi = create<UiState>((set) => ({
  sidebarOpen: true,
  graphOverlayOpen: false,
  selectedSessionId: null,
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
  toggleGraphOverlay: () => set((s) => ({ graphOverlayOpen: !s.graphOverlayOpen })),
  setSelectedSession: (id) => set({ selectedSessionId: id }),
}));
```

### 8. 태그 색상 해시

`web/src/lib/tagColor.ts`:
```ts
const PALETTE = [
  "bg-violet-500/15 text-violet-300 ring-violet-500/30",
  "bg-cyan-500/15 text-cyan-300 ring-cyan-500/30",
  "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30",
  "bg-amber-500/15 text-amber-300 ring-amber-500/30",
  "bg-rose-500/15 text-rose-300 ring-rose-500/30",
  "bg-blue-500/15 text-blue-300 ring-blue-500/30",
  "bg-fuchsia-500/15 text-fuchsia-300 ring-fuchsia-500/30",
  "bg-teal-500/15 text-teal-300 ring-teal-500/30",
];

export function tagColor(tag: string): string {
  let hash = 0;
  for (let i = 0; i < tag.length; i++) hash = (hash * 31 + tag.charCodeAt(i)) | 0;
  return PALETTE[Math.abs(hash) % PALETTE.length];
}
```

### 9. 라우터

`web/src/routes/router.tsx`:
```tsx
import { createBrowserRouter, Navigate } from "react-router";
import Layout from "./Layout";
import SessionsRoute from "./SessionsRoute";
import SessionDetailRoute from "./SessionDetailRoute";
import DailyRoute from "./DailyRoute";
import WikiRoute from "./WikiRoute";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <Layout />,
    children: [
      { index: true, element: <Navigate to="/sessions" replace /> },
      { path: "sessions", element: <SessionsRoute /> },
      { path: "sessions/:id", element: <SessionDetailRoute /> },
      { path: "daily", element: <DailyRoute /> },
      { path: "daily/:date", element: <DailyRoute /> },
      { path: "wiki", element: <WikiRoute /> },
      { path: "wiki/:project", element: <WikiRoute /> },
    ],
  },
]);
```

### 10. main.tsx (Provider 트리)

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router/dom";
import { Toaster } from "@/components/ui/sonner";
import { router } from "@/routes/router";
import { queryClient } from "@/lib/queryClient";
import "./index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster theme="dark" />
    </QueryClientProvider>
  </StrictMode>,
);
```

`web/src/lib/queryClient.ts`:
```ts
import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 30_000, refetchOnWindowFocus: false },
  },
});
```

### 11. Layout placeholder

`web/src/routes/Layout.tsx`:
```tsx
import { NavLink, Outlet } from "react-router";
import { Search, Calendar, Network, BookOpen } from "lucide-react";

const NAV = [
  { to: "/sessions", icon: Search, label: "Sessions" },
  { to: "/daily", icon: Calendar, label: "Daily" },
  { to: "/wiki", icon: BookOpen, label: "Wiki" },
];

export default function Layout() {
  return (
    <div className="flex h-screen bg-background text-foreground">
      <aside className="w-56 border-r border-border p-4 space-y-1">
        <div className="text-lg font-semibold mb-6">seCall</div>
        {NAV.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-2 px-3 py-2 rounded-md text-sm ${
                isActive ? "bg-accent text-accent-foreground" : "hover:bg-accent/50 text-muted-foreground"
              }`
            }
          >
            <Icon className="size-4" /> {label}
          </NavLink>
        ))}
      </aside>
      <main className="flex-1 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
```

### 12. 라우트 placeholder들

각 SessionsRoute/SessionDetailRoute/DailyRoute/WikiRoute는 단순 텍스트만 ("Sessions placeholder — Task 06") — 후속 task에서 본격 구현.

`web/src/routes/GraphRoute.tsx`는 라우터에 등록하지 않음 (그래프는 오버레이로만 노출 — Task 08).

## Dependencies

- Task 01 완료 (워크스페이스 + 빌드 파이프라인)

## Verification

```bash
# 1. 의존성 설치
cd web && pnpm install

# 2. shadcn 컴포넌트 생성 (init은 components.json 있으면 skip)
cd web && pnpm dlx shadcn@latest add button input sheet badge dialog card separator scroll-area sonner

# 3. 타입 체크
cd web && pnpm typecheck

# 4. 빌드 성공
cd web && pnpm build && test -f dist/index.html

# 5. dev 모드 — 별도 터미널에 `cargo run -- serve --port 8080` 띄운 상태에서
cd web && pnpm dev &
DEV_PID=$!
sleep 3
curl -s http://127.0.0.1:5173/ | grep -qi "secall" && echo "vite dev OK"
kill $DEV_PID 2>/dev/null || true

# 6. # Manual: http://127.0.0.1:5173 접속 →
#   - 좌측 사이드바에 "Sessions / Daily / Wiki" 메뉴 보임
#   - "/sessions" 라우트로 자동 이동 (placeholder 텍스트 보임)
#   - 다크 모드 적용됨
#   - Pretendard 폰트로 한글 렌더 (메뉴는 영문이지만 페이지 내 한글 텍스트로 확인)
```

## Risks

- **shadcn 버전 호환성**: shadcn CLI 버전에 따라 `add` 명령 동작 차이 — 본 task에서 명시 버전 고정 필요 시 README 안내
- **Pretendard CDN 의존**: 오프라인 환경에서 폰트 로드 실패. 필요 시 `web/public/fonts/`에 호스팅 (Phase 1+)
- **bundle 크기**: 모든 의존성 합쳐 약 300-500KB gzip 예상. rust-embed 임베드 시 바이너리 크기 증가 — 허용 범위
- **React 18 vs 19**: react-router v7은 React 18/19 모두 지원. 본 task는 React 18로 시작
- **TanStack Query staleTime 30s**: 너무 길면 태그 변경 후 즉시 반영 안 될 수 있음 — invalidate로 해결 가능 (Task 07)
- **다크 모드 토글 미제공**: MVP는 다크 고정. 라이트 모드는 v1.1
- **Geist Sans CDN 경로**: `npm:geist@1.3.1`은 npm 패키지지만 CSS만 가져옴. CDN 안정성 확인 필요 — 실패 시 system-ui 폴백 동작

## Scope boundary

수정 금지:
- `crates/` — Rust 영역 전체 (Task 02, 03, 04에서 다룸)
- `obsidian-secall/` — 별도 트랙
- `.github/workflows/`, `README.md` — Task 09
- `web/src/routes/Layout.tsx`의 그래프 토글 — Task 08
- `web/src/routes/SessionsRoute.tsx`, `SessionDetailRoute.tsx` 본격 구현 — Task 06
- `web/src/routes/DailyRoute.tsx`, `WikiRoute.tsx` 본격 구현 — Task 07
