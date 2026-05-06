default:
    @just --list

# 프로덕션 빌드 (web → cargo build --release)
build: web
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
