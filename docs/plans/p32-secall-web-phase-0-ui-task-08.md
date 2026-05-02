---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 08
parallel_group: F
depends_on: [01]
---

# Task 08 — README + CI 업데이트

## Changed files

수정:
- `.github/workflows/ci.yml` — Node 22 + pnpm setup + `pnpm build` 단계 추가
- `.github/workflows/release.yml` — 각 빌드 매트릭스에 Node + pnpm + `pnpm build` 추가 (web/dist 생성 후 cargo build)
- `README.md` — 웹 UI 섹션 추가, 설치 옵션 (release binary / cargo install / brew tap), `secall serve` 안내 갱신
- `README.en.md` — 동일 내용 영문판
- `crates/secall/Cargo.toml` — `web-ui` feature flag 추가 (`default = ["web-ui"]`), `web-ui = []` 빈 feature
- `crates/secall-core/Cargo.toml` — `web-ui` feature flag 추가, `rust-embed`/`mime_guess` deps를 optional로 (선택 — `cargo install secall --no-default-features --features cli-only` 지원 위해)
- `crates/secall-core/src/lib.rs:1-10` — `#[cfg(feature = "web-ui")] pub mod web;` 가드
- `crates/secall-core/src/mcp/rest.rs:101-110` — web router merge를 feature-gate

신규: 없음

## Change description

### 1. `web-ui` feature flag

`cargo install secall`은 npm 빌드 자동 수행 못 하므로 web 자산 미포함 빌드 옵션 필요.

`crates/secall-core/Cargo.toml`:
```toml
[dependencies]
# ...
rust-embed = { workspace = true, optional = true }
mime_guess = { workspace = true, optional = true }

[features]
openvino = ["dep:openvino", "dep:libloading"]
web-ui = ["dep:rust-embed", "dep:mime_guess"]
```

`crates/secall/Cargo.toml`:
```toml
[features]
default = ["web-ui"]
web-ui = ["secall-core/web-ui"]
openvino = ["secall-core/openvino"]
```

`crates/secall-core/src/lib.rs`:
```rust
#[cfg(feature = "web-ui")]
pub mod web;
```

`crates/secall-core/src/mcp/rest.rs`의 `rest_router()`:
```rust
let api = Router::new()
    .route("/api/recall", post(api_recall))
    // ... 기타 엔드포인트
    .layer(cors)
    .with_state(state);

#[cfg(feature = "web-ui")]
let api = api.merge(crate::web::web_router());

api
```

> Task 02에서 작성한 `crate::web::web_router()` 호출이 web-ui feature 없으면 컴파일 안 되므로 cfg-gate 필수.

### 2. CI workflow 업데이트

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-Dwarnings"
  FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true

jobs:
  web-build:
    name: Build web/dist
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - uses: pnpm/action-setup@v4
        with:
          version: 9
      - name: Cache pnpm store
        uses: actions/cache@v4
        with:
          path: ~/.local/share/pnpm/store
          key: ${{ runner.os }}-pnpm-${{ hashFiles('web/pnpm-lock.yaml') }}
          restore-keys: ${{ runner.os }}-pnpm-
      - name: Install + typecheck + build
        working-directory: web
        run: |
          pnpm install --frozen-lockfile
          pnpm typecheck
          pnpm build
      - uses: actions/upload-artifact@v4
        with:
          name: web-dist
          path: web/dist
          retention-days: 1

  check:
    name: Check & Test
    needs: web-build
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - name: Cache cargo registry & build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: ${{ runner.os }}-cargo-
      - name: Download web/dist
        uses: actions/download-artifact@v4
        with:
          name: web-dist
          path: web/dist
      - name: cargo fmt --check
        run: cargo fmt --all -- --check
      - name: cargo clippy
        run: cargo clippy --all-targets --all-features
      - name: cargo test
        run: cargo test --all
      - name: cargo audit
        run: |
          cargo install cargo-audit --locked || true
          cargo audit
        continue-on-error: true
```

### 3. Release workflow 업데이트

`.github/workflows/release.yml` 각 매트릭스 빌드 step 앞에 web 빌드 추가:
```yaml
jobs:
  web-build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 22 }
      - uses: pnpm/action-setup@v4
        with: { version: 9 }
      - working-directory: web
        run: |
          pnpm install --frozen-lockfile
          pnpm build
      - uses: actions/upload-artifact@v4
        with:
          name: web-dist
          path: web/dist

  build:
    needs: web-build
    strategy:
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-apple-darwin
            os: macos-14
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: actions/download-artifact@v4
        with:
          name: web-dist
          path: web/dist
      - run: cargo build --release --target ${{ matrix.target }} -p secall
      # 이하 기존 Bundle ORT DLL / Package / upload-artifact 동일
```

### 4. README.md 추가/수정

추가 섹션 "Web UI":
```markdown
## Web UI

`secall serve`는 REST API와 함께 웹 UI를 동일 포트에서 제공합니다 (단일 진입점).

### 사용법

```bash
secall serve --port 8080
# 브라우저에서 http://127.0.0.1:8080 접속
```

기능 (Phase 0):
- 검색 / 세션 브라우징 (2-pane 레이아웃)
- 일일 일기 / 위키 페이지 열람
- 그래프 탐색 (사이드바 Graph 버튼 → 풀스크린 오버레이)
- 태그 / 즐겨찾기 편집

명령 트리거 (sync/ingest/wiki update)는 Phase 1에서 추가 예정.
```

설치 섹션:
```markdown
## 설치

### 1. GitHub Releases (권장)

[Releases 페이지](https://github.com/hang-in/seCall/releases)에서 OS에 맞는 바이너리 다운로드.

웹 UI 포함된 단일 바이너리입니다.

### 2. Homebrew (macOS)

```bash
brew install hang-in/tap/secall
```

> tap 등록은 별도 작업 — TODO

### 3. Cargo (개발자)

```bash
# 웹 UI 미포함 (CLI/MCP/REST API만)
cargo install secall --no-default-features

# 웹 UI 포함 — pnpm + Node 22 사전 설치 + web/dist 빌드 필요
git clone https://github.com/hang-in/seCall && cd seCall
just build
cp target/release/secall ~/.local/bin/
```

> `cargo install`은 npm 빌드를 자동으로 하지 않으므로 웹 UI는 Releases 바이너리 또는 직접 빌드 권장.
```

개발 섹션:
```markdown
## 개발

### 사전 요구사항

- Rust stable (1.75+)
- Node 22 + pnpm 9
- [just](https://just.systems) (선택)

### 빌드

```bash
just build           # web + cargo --release
# 또는 수동:
cd web && pnpm install && pnpm build && cd ..
cargo build --release
```

### 개발 모드

```bash
just dev             # Vite dev server (5173) + cargo run (8080)
# 브라우저에서 http://127.0.0.1:8080 또는 5173 접속
```

`just dev`는 Vite를 5173에서 띄우고, 8080에서 axum이 reverse proxy합니다.
- 8080 접속: 단일 포트로 모든 것 동작 (HMR은 새로고침 필요)
- 5173 직접 접속: HMR 동작, `/api/*`는 8080으로 프록시됨
```

### 5. README.en.md

위 한글 섹션을 영문으로 동등하게 추가.

### 6. CI 캐시 정책

- `actions/cache@v4`로 pnpm store 캐시 (`~/.local/share/pnpm/store`)
- web-build job → check job 사이에 `actions/upload-artifact` + `actions/download-artifact`로 dist 전달
- artifact retention: 1일 (CI 임시용)

## Dependencies

- Task 02 완료 (web router 코드 존재, feature-gate 적용 가능)

## Verification

```bash
# 1. cargo feature flag 동작 확인
cargo check -p secall --no-default-features  # web-ui 없이도 컴파일
cargo check -p secall                         # default = web-ui 포함
cargo check -p secall --features openvino     # 다른 feature와 조합

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. CI workflow YAML 문법 (yamllint or actionlint 있으면)
# Manual: GitHub UI에서 워크플로우 보기 또는 actionlint 사용
which actionlint && actionlint .github/workflows/ci.yml .github/workflows/release.yml || echo "actionlint not installed — manual check"

# 4. README 렌더 확인
# Manual: GitHub UI 또는 `glow README.md` (glow 설치 시) 으로 마크다운 렌더 확인
test -f README.md && grep -q "Web UI" README.md && echo "README updated"
test -f README.en.md && grep -q "Web UI" README.en.md && echo "README.en updated"

# 5. PR 생성 후 GitHub Actions 로그에서 web-build job 통과 + check job이 web/dist 받는지 확인
# Manual: 실제 CI run 결과 확인 (Task 09 완료 후 메인 머지 직전)

# 6. Release dry-run (tag 안 만들고)
# Manual: release.yml은 tag 트리거이므로 실제 검증은 다음 release 시
```

## Risks

- **web-ui feature 분리의 복잡도**: feature gate를 빠뜨리면 컴파일 에러. Task 02에서 작성한 `crate::web::web_router()` 호출 경로 모두 cfg-gate 필요. clippy로 확인
- **CI 캐시 무효화**: `pnpm-lock.yaml` 변경 시 캐시 키 바뀜 — 정상 동작
- **Windows pnpm 호환성**: pnpm은 Windows에서 잘 동작하지만 path 문제 종종 발생. CI는 Linux에서 web 빌드 후 artifact 전달이라 안전
- **release workflow의 macOS 두 빌드가 web-dist 다운로드**: artifact 다운로드 1번 (전체 매트릭스 공유 가능 여부 확인 — `actions/download-artifact@v4`는 같은 workflow 내에서는 OK)
- **Homebrew tap 부재**: README에 안내했지만 실제 tap repository는 미존재. README에 `> TODO: tap 등록 예정` 명시
- **`cargo install secall` 사용자 혼란**: 기본은 `--no-default-features` 아니라 `default = ["web-ui"]`이라 cargo install 시 컴파일 실패 가능. README에 명확히 안내. 실제 published crate에서는 `cargo install secall`이 web-ui 없이 빌드되도록 default를 변경하는 옵션도 검토 — 본 task는 default = web-ui로 두되 README 안내로 처리
- **AGPL + xyflow attribution**: Task 08의 `proOptions: { hideAttribution: true }`가 Pro 라이선스 필요할 수 있음. 본 task에서 라이선스 검토 후 attribution 표시 결정

## Scope boundary

수정 금지:
- `crates/secall-core/src/web/`, `mcp/rest.rs`의 라우트/핸들러 본체 — Task 02, 03
- `web/src/` 코드 — Task 05~08
- DB 스키마 — Task 04
- 기존 마이그레이션 분기 변경 — Task 04에서 v5 분기만 추가했어야 함
