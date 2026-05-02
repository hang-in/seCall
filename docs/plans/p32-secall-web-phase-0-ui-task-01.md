---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p32-secall-web-phase-0-ui
task_id: 01
parallel_group: B
depends_on: [00]
---

# Task 01 — rust-embed + Vite reverse proxy 통합

## Changed files

신규:
- `crates/secall-core/src/web/mod.rs` — web router 모듈 (release/debug 분기)
- `crates/secall-core/src/web/embed.rs` — `rust-embed` 임베드 (release 전용 코드)
- `crates/secall-core/src/web/proxy.rs` — Vite reverse proxy (debug 전용 코드)

수정:
- `crates/secall-core/src/lib.rs:1-10` — `pub mod web;` 추가
- `crates/secall-core/src/mcp/rest.rs:101-110` — `rest_router()` 끝에 web router merge
- `crates/secall-core/Cargo.toml:7-40` — `rust-embed`, `mime_guess` 의존성 추가
- `Cargo.toml` (workspace 루트) — `rust-embed`, `mime_guess` workspace 의존성 등록

## Change description

### 1. 의존성 추가

`Cargo.toml` (workspace 루트):
```toml
rust-embed = "8"
mime_guess = "2"
```

`crates/secall-core/Cargo.toml`:
```toml
rust-embed = { workspace = true }
mime_guess = { workspace = true }
```

### 2. `crates/secall-core/src/web/mod.rs` 신규

```rust
//! 정적 웹 자산 서빙 모듈.
//!
//! Release 빌드: `rust-embed`으로 `web/dist/`를 바이너리에 임베드.
//! Debug 빌드: Vite dev server (`http://127.0.0.1:5173`)로 reverse proxy.

use axum::Router;

#[cfg(not(debug_assertions))]
mod embed;

#[cfg(debug_assertions)]
mod proxy;

pub fn web_router() -> Router {
    #[cfg(not(debug_assertions))]
    {
        embed::router()
    }
    #[cfg(debug_assertions)]
    {
        proxy::router()
    }
}
```

### 3. `crates/secall-core/src/web/embed.rs` (release 전용)

```rust
use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist/"]
struct Assets;

pub fn router() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/*path", get(serve_asset))
}

async fn serve_index() -> Response {
    serve_path("index.html")
}

async fn serve_asset(Path(path): Path<String>) -> Response {
    // SPA fallback: 정적 자산이 아니면 index.html
    if Assets::get(&path).is_some() {
        serve_path(&path)
    } else {
        serve_path("index.html")
    }
}

fn serve_path(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    }
}
```

> SPA fallback: `react-router` 클라이언트 라우팅 지원을 위해 임의 경로는 `index.html` 반환.

### 4. `crates/secall-core/src/web/proxy.rs` (debug 전용)

```rust
use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    Router,
};

const VITE_DEV_URL: &str = "http://127.0.0.1:5173";

pub fn router() -> Router {
    Router::new().fallback(proxy_handler)
}

async fn proxy_handler(req: Request) -> Response {
    let path = req.uri().path();
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let target = format!("{VITE_DEV_URL}{path}{query}");

    let client = reqwest::Client::new();
    let method = req.method().clone();
    let headers = req.headers().clone();

    // body 추출
    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => return error_response(format!("body read failed: {e}")),
    };

    let mut request = client.request(method, &target).body(body_bytes.to_vec());
    for (k, v) in headers.iter() {
        if k != "host" {
            request = request.header(k, v);
        }
    }

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = resp.headers().clone();
            let body = resp.bytes().await.unwrap_or_default();
            let mut builder = Response::builder().status(status);
            for (k, v) in resp_headers.iter() {
                if k != "transfer-encoding" && k != "connection" {
                    builder = builder.header(k, v);
                }
            }
            builder.body(Body::from(body)).unwrap()
        }
        Err(e) => error_response(format!(
            "Vite dev server unreachable at {VITE_DEV_URL}: {e}\n\
             Run `cd web && pnpm dev` in another terminal."
        )),
    }
}

fn error_response(msg: String) -> Response {
    (StatusCode::BAD_GATEWAY, msg).into_response()
}
```

> 주의: WebSocket 업그레이드(Vite HMR)는 별도 처리 필요. MVP에서는 HMR이 5173 직접 접속(브라우저)으로 동작하면 충분 — 8080으로 접속한 경우 페이지 새로고침 필요. WebSocket 프록시는 v1.1.

### 5. `crates/secall-core/src/mcp/rest.rs:101-110` 수정

기존 `rest_router()` 끝에 web router merge:
```rust
pub fn rest_router(server: SeCallMcpServer) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let state: AppState = Arc::new(server);

    let api = Router::new()
        .route("/api/recall", post(api_recall))
        .route("/api/get", post(api_get))
        .route("/api/status", get(api_status))
        .route("/api/wiki", post(api_wiki))
        .route("/api/graph", post(api_graph))
        .route("/api/daily", post(api_daily))
        .layer(cors)
        .with_state(state);

    // web router는 fallback으로 (api 경로가 우선 매칭됨)
    api.merge(crate::web::web_router())
}
```

### 6. `crates/secall-core/src/lib.rs` 수정

```rust
pub mod error;
pub mod graph;
pub mod hooks;
pub mod ingest;
pub mod mcp;
pub mod search;
pub mod store;
pub mod vault;
pub mod web;       // ← 추가
pub mod wiki;
```

## Dependencies

- Task 01 완료 (`web/dist/` 디렉토리가 빌드 시 존재해야 release embed 가능)
- 외부 crate: `rust-embed = "8"`, `mime_guess = "2"`
- 기존: `reqwest`, `axum`, `tower-http` (이미 있음)

## Verification

```bash
# 1. cargo 의존성 추가 확인
grep -q "rust-embed" Cargo.toml && grep -q "rust-embed" crates/secall-core/Cargo.toml && echo "deps OK"

# 2. web/dist/ 미리 빌드
cd web && pnpm install --frozen-lockfile && pnpm build && cd ..

# 3. release 빌드 (rust-embed 포함)
cargo build --release -p secall

# 4. release 바이너리에 자산이 임베드됐는지 (크기 확인 — 임베드 전 대비 200KB+ 증가 예상)
ls -lh target/release/secall

# 5. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 6. release 모드에서 정적 자산 서빙 검증
./target/release/secall serve --port 18080 &
SERVER_PID=$!
sleep 2
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:18080/    # 200 기대
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:18080/api/status  # 200 기대 (기존 API)
kill $SERVER_PID 2>/dev/null || true

# 7. # Manual: dev 모드 — 별도 터미널에 `cd web && pnpm dev` 띄우고
#   `cargo run -- serve --port 8080` 실행 후
#   http://127.0.0.1:8080 접속 → "seCall web — Phase 0 (Task 01 placeholder)" 보이는지 확인
```

## Risks

- **rust-embed 매크로 경로**: `#[folder = "../../web/dist/"]`은 `crates/secall-core/`에서 `../../web/dist/`. 워크스페이스 루트 기준 `web/dist/`. 빌드 시 디렉토리 부재 시 컴파일 에러 — Task 01의 `just build`가 web 빌드를 먼저 수행해야 함
- **Reverse proxy WebSocket 미지원**: Vite HMR이 8080 경유로는 동작 안 함. 개발자가 5173 직접 접속하거나 8080에서는 수동 새로고침 필요. README에 명시 (Task 09)
- **proxy.rs body 읽기 메모리**: `to_bytes(_, usize::MAX)`는 대용량 요청에 위험하지만 dev 모드 한정이라 실용적 문제 없음
- **mime_guess 부정확성**: 일부 .map 등 비표준 확장자는 octet-stream로 폴백. 주요 자산(JS/CSS/HTML/SVG/PNG)은 정확
- **debug 빌드는 web/dist 불필요**: proxy 모드에서는 dist 디렉토리 없어도 컴파일됨 — `#[cfg(not(debug_assertions))]` 분기로 보장

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/` — Task 03, 04
- `crates/secall-core/src/mcp/server.rs` — 본 task에서 변경 없음 (rest.rs에서 router만 merge)
- `crates/secall/src/commands/serve.rs` — 본 task에서 변경 없음 (rest_router가 web을 포함하도록 했으므로 serve 진입점은 그대로)
- `web/` 내부 — Task 01, 05
- `.github/workflows/` — Task 09
