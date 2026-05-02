---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 00
parallel_group: A
depends_on: []
---

# Task 00 — 워크스페이스 + 빌드 파이프라인

## Changed files

신규:
- `web/package.json` — pnpm 패키지 정의
- `web/pnpm-lock.yaml` — pnpm lock (`pnpm install` 자동 생성)
- `web/vite.config.ts` — Vite 설정 (포트 5173, `/api/*` 프록시 → 8080)
- `web/tsconfig.json` — TypeScript 설정
- `web/tsconfig.node.json` — Vite 설정용 TypeScript
- `web/index.html` — Vite 진입 HTML
- `web/src/main.tsx` — React 부트스트랩 (placeholder, 5번에서 본격 구성)
- `web/src/App.tsx` — placeholder ("seCall web — Phase 0")
- `web/.gitignore` — `dist/`, `node_modules/`
- `justfile` — 빌드/실행 명령 통합 (루트)
- `.tool-versions` — Node 22 / pnpm 9 (asdf/mise 호환, 선택)

수정:
- `.gitignore` (루트) — `web/dist/`, `web/node_modules/`, `.DS_Store` 추가 (이미 있는지 확인)

## Change description

### 1. `web/` 디렉토리 생성

`obsidian-secall/`과 같은 레벨에 `web/` 신규. 디렉토리 구조:
```
web/
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json
├── tsconfig.node.json
├── index.html
├── .gitignore
└── src/
    ├── main.tsx
    └── App.tsx
```

### 2. `package.json` 의존성

스택 결정 (P32 플랜 F항):
```json
{
  "name": "secall-web",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "typecheck": "tsc --noEmit",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-router": "^7.0.0"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.3",
    "typescript": "^5.6.3",
    "vite": "^5.4.10"
  }
}
```

> 참고: Zustand / TanStack Query / Tailwind / shadcn 등은 Task 05에서 추가. Task 01은 빌드 파이프라인 골격만.

### 3. `vite.config.ts`

dev server에서 `/api/*` 요청을 `http://127.0.0.1:8080`으로 프록시:
```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8080",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
```

> 주의: 프로덕션 모드에서는 Task 02의 axum이 정적 자산을 서빙하므로 이 프록시는 dev 전용. 단, dev 모드에서 사용자가 5173에 직접 접속할 수도, 8080에 접속할 수도 있음 — 두 경우 모두 동작해야 함 (Task 02에서 axum reverse proxy로 통일).

### 4. `tsconfig.json`

표준 React + Vite 설정:
```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

### 5. `index.html`

```html
<!doctype html>
<html lang="ko">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>seCall</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

### 6. `web/src/main.tsx` + `App.tsx` (placeholder)

`main.tsx`:
```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
```

`App.tsx`:
```tsx
export default function App() {
  return <div>seCall web — Phase 0 (Task 01 placeholder)</div>;
}
```

> Task 05에서 React Router / Zustand / TanStack Query / shadcn으로 본격 교체.

### 7. `justfile` (루트)

빌드/실행 명령 통합:
```
default:
    @just --list

# 프로덕션 빌드 (web → cargo build --release)
build:
    cd web && pnpm install --frozen-lockfile && pnpm build
    cargo build --release

# 개발 모드: Vite dev server + axum 둘 다 띄움
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    (cd web && pnpm dev) &
    VITE_PID=$!
    trap "kill $VITE_PID 2>/dev/null || true" EXIT
    cargo run -- serve --port 8080

# 타입 체크 + 린트 + 테스트
check:
    cd web && pnpm typecheck
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features
    cargo test --all

# web만 빌드
web:
    cd web && pnpm install --frozen-lockfile && pnpm build
```

> `just dev`는 두 프로세스를 띄우되, justfile EXIT 트랩으로 Vite 자식 프로세스 정리. 개발자가 Ctrl+C로 cargo 종료 시 Vite도 함께 종료됨.

### 8. `.gitignore` 업데이트

루트 `.gitignore`에 추가:
```
web/dist/
web/node_modules/
```

`web/.gitignore`에:
```
dist/
node_modules/
*.local
```

## Dependencies

- 외부: Node 22+, pnpm 9+, just (https://just.systems)
- 내부 task: 없음 (root)

## Verification

```bash
# 1. just 명령이 보이는지
just --list

# 2. 디렉토리/파일 존재 확인
test -d web && test -f web/package.json && test -f web/vite.config.ts && test -f justfile && echo "files OK"

# 3. pnpm 의존성 설치 + 타입 체크
cd web && pnpm install && pnpm typecheck

# 4. Vite 빌드 성공
cd web && pnpm build && test -f dist/index.html && echo "vite build OK"

# 5. justfile 빌드 명령 동작 (web만)
just web

# 6. Rust 측 영향 없음 확인
cargo check --all-targets --all-features
```

## Risks

- **Lock 파일 충돌**: `pnpm-lock.yaml` 처음 생성 시 commit 필요. CI에서 `--frozen-lockfile`로 재현성 보장
- **Node 버전 불일치**: 개발자 환경마다 Node 버전 다를 수 있음 — `.tool-versions`로 명시
- **just 미설치 환경**: `just`가 macOS/Linux에서 brew/apt로 설치 가능하지만 사용자가 못 깔 수 있음. README에 설치 안내 필요 (Task 09)
- **포트 충돌**: 5173, 8080이 다른 프로세스와 충돌 가능. 문서화

## Scope boundary

수정 금지 (다른 task 영역):
- `crates/secall-core/src/mcp/rest.rs` — Task 02, 03
- `crates/secall-core/src/store/schema.rs`, `db.rs` — Task 04
- `crates/secall-core/src/store/session_repo.rs` — Task 03, 04
- `web/src/` 내부 본격 React 코드 (main.tsx/App.tsx의 placeholder 외) — Task 05
- `.github/workflows/` — Task 09
- `README.md`, `README.en.md` — Task 09
