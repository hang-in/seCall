use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    Router,
};

const VITE_DEV_URL: &str = "http://127.0.0.1:5173";

pub fn router() -> Router {
    Router::new().fallback(proxy_handler)
}

async fn proxy_handler(req: Request) -> Response {
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let target = format!("{VITE_DEV_URL}{path}{query}");

    let client = reqwest::Client::new();
    let method = req.method().clone();
    let headers = req.headers().clone();

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
