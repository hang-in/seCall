use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::Response,
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
        .route("/{*path}", get(serve_asset))
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
