//! 빌드 스크립트 — `web-ui` 기능의 정적 자산 임베드를 견고하게 만든다.
//!
//! `src/web/embed.rs` 의 `#[derive(RustEmbed)] #[folder = "../../web/dist/"]` 는
//! release 빌드 시점에 `web/dist/` 폴더가 **반드시 존재**해야 컴파일된다.
//! `web/dist/` 는 `pnpm build` 산출물이라 `.gitignore` 대상이고 git 에 포함되지
//! 않으므로, 외부 사용자가 `cargo install --git ...` 로 받으면 폴더가 없어
//! `RustEmbed` derive 가 컴파일 에러를 낸다.
//!
//! 이 스크립트는 두 가지를 보장한다:
//!   1. `web/dist/index.html` 변경 시 cargo 가 rebuild 하도록 추적한다.
//!      (dist 갱신 후 `cargo install` 만 돌리면 옛 번들이 embed 되던 회귀 방지)
//!   2. `web/dist/` 가 없으면 안내용 placeholder 를 만들어 컴파일이 깨지지 않게
//!      한다. CLI · MCP · REST API 는 그대로 동작하고, 웹 UI 자리에는 빌드 방법을
//!      안내하는 페이지가 표시된다.

use std::path::Path;

fn main() {
    // `web-ui` 기능이 꺼져 있으면 embed 자체가 컴파일되지 않으므로 할 일이 없다.
    // cargo 는 활성 기능마다 `CARGO_FEATURE_<NAME>` (대문자, `-`→`_`) 을 설정한다.
    if std::env::var_os("CARGO_FEATURE_WEB_UI").is_none() {
        return;
    }

    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    // embed.rs 의 `#[folder = "../../web/dist/"]` 와 동일한 위치.
    let dist_dir = Path::new(&manifest_dir).join("../../web/dist");
    let index_html = dist_dir.join("index.html");

    // dist 가 바뀌면 (특히 index.html) cargo 가 재컴파일하도록 추적한다.
    println!("cargo:rerun-if-changed={}", index_html.display());

    if index_html.exists() {
        // 정상 경로: CI / `just build` / `pnpm build` 로 만들어진 실제 번들이 있다.
        return;
    }

    // dist 가 없다 — placeholder 를 만들어 컴파일을 살린다.
    if let Err(e) = std::fs::create_dir_all(&dist_dir) {
        // 디렉터리조차 못 만들면 RustEmbed 가 명확히 에러를 내도록 그냥 둔다.
        println!("cargo:warning=secall: web/dist 생성 실패 ({e}). 웹 UI 없이 빌드하려면 --no-default-features 를 사용하세요.");
        return;
    }

    if let Err(e) = std::fs::write(&index_html, PLACEHOLDER_INDEX_HTML) {
        println!("cargo:warning=secall: placeholder index.html 작성 실패 ({e}).");
        return;
    }

    println!(
        "cargo:warning=secall: web/dist 가 없어 placeholder 웹 UI 로 빌드합니다. \
         실제 웹 UI 를 쓰려면 `cd web && pnpm install && pnpm build` 후 다시 빌드하거나, \
         웹 UI 가 필요 없으면 `--no-default-features` 로 설치하세요."
    );
}

/// dist 가 없을 때 임베드되는 안내 페이지.
const PLACEHOLDER_INDEX_HTML: &str = r#"<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>seCall — web UI not built</title>
<style>
  body { font-family: system-ui, sans-serif; max-width: 42rem; margin: 4rem auto; padding: 0 1.5rem; line-height: 1.6; color: #1a1a1a; }
  code { background: #f0f0f0; padding: .15em .4em; border-radius: 4px; }
  pre { background: #f6f8fa; padding: 1rem; border-radius: 8px; overflow-x: auto; }
  h1 { font-size: 1.4rem; }
  .muted { color: #666; }
</style>
</head>
<body>
  <h1>seCall web UI is not built</h1>
  <p>This binary was compiled without a pre-built <code>web/dist/</code> bundle, so the web UI is a placeholder. The REST API, MCP server, and CLI all work normally.</p>
  <p>To get the full web UI, build the frontend and reinstall:</p>
  <pre>cd web
pnpm install
pnpm build
# then rebuild/reinstall secall</pre>
  <p class="muted">If you don't need the web UI, install with <code>--no-default-features</code> to skip it entirely.</p>
</body>
</html>
"#;
