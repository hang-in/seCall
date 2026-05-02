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
