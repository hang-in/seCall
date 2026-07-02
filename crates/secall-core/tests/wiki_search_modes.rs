use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::post, Json, Router};
use secall_core::{
    mcp::{
        tools::{WikiSearchMode, WikiSearchParams},
        SeCallMcpServer,
    },
    search::{
        vector::VectorIndexer, Bm25Indexer, LinderaKoTokenizer, OllamaEmbedder, SearchEngine,
    },
    store::{Database, WikiVectorRepo},
};
use serde_json::json;

/// 쿼리 임베딩용 벡터 인덱서를 stub ollama(embed_url)로 구성해 서버를 만든다.
/// embed_url=None 이면 벡터 검색 비활성(키워드 전용).
///
/// (#121 이후 위키 시맨틱 검색은 env(OLLAMA_BASE_URL) 가 아니라 `SearchEngine` 의 config
/// 벡터 인덱서 경로로 쿼리를 임베딩하므로, 테스트도 stub 을 인덱서로 주입한다.)
fn make_server(
    vault_path: &std::path::Path,
    db: Database,
    embed_url: Option<&str>,
) -> SeCallMcpServer {
    let tok = LinderaKoTokenizer::new().expect("tokenizer init");
    let vector =
        embed_url.map(|url| VectorIndexer::new(Box::new(OllamaEmbedder::new(Some(url), None))));
    let engine = SearchEngine::new(Bm25Indexer::new(Box::new(tok)), vector);
    SeCallMcpServer::new(
        Arc::new(Mutex::new(db)),
        Arc::new(engine),
        vault_path.to_path_buf(),
    )
}

fn write_page(tmp: &tempfile::TempDir, rel_path: &str, body: &str) {
    let path = tmp.path().join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, body).expect("write wiki page");
}

async fn start_embed_stub(
    embeddings: Arc<Mutex<Vec<Vec<f32>>>>,
) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/api/embed",
            post(
                |State(embeddings): State<Arc<Mutex<Vec<Vec<f32>>>>>,
                 Json(_body): Json<serde_json::Value>| async move {
                    let embeddings = embeddings.lock().expect("embeddings lock").clone();
                    Json(json!({ "embeddings": embeddings }))
                },
            ),
        )
        .with_state(embeddings);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve stub");
    });
    (format!("http://{addr}"), handle)
}

#[test]
fn test_keyword_mode_default_when_mode_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_page(
        &tmp,
        "wiki/projects/keyword.md",
        "# Keyword Page\n\nrust exact keyword match",
    );

    let server = make_server(tmp.path(), Database::open_memory().expect("db"), None);
    let json = server
        .do_wiki_search(WikiSearchParams {
            query: "keyword".to_string(),
            category: None,
            limit: Some(5),
            mode: None,
        })
        .expect("wiki search");

    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["path"], "wiki/projects/keyword.md");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_semantic_mode_returns_results() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_page(
        &tmp,
        "wiki/projects/semantic.md",
        "# Semantic Page\n\nThis page explains git automation flows.",
    );

    let db = Database::open_memory().expect("db");
    db.upsert_wiki_vector(
        "wiki/projects/semantic.md",
        &[1.0, 0.0, 0.0, 0.0],
        "bge-m3",
        "hash-semantic",
    )
    .expect("insert semantic row");

    let embeddings = Arc::new(Mutex::new(vec![vec![1.0, 0.0, 0.0, 0.0]]));
    let (base_url, handle) = start_embed_stub(embeddings).await;

    let server = make_server(tmp.path(), db, Some(&base_url));
    let json = server
        .do_wiki_search(WikiSearchParams {
            query: "git automation".to_string(),
            category: None,
            limit: Some(5),
            mode: Some(WikiSearchMode::Semantic),
        })
        .expect("semantic search");

    handle.abort();

    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["path"], "wiki/projects/semantic.md");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_hybrid_mode_combines_both() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_page(
        &tmp,
        "wiki/projects/keyword-hit.md",
        "# Keyword Hit\n\ngit 자동화 keyword only phrase",
    );
    write_page(
        &tmp,
        "wiki/projects/semantic-hit.md",
        "# Semantic Hit\n\nvault auto commit workflow page",
    );

    let db = Database::open_memory().expect("db");
    db.upsert_wiki_vector(
        "wiki/projects/semantic-hit.md",
        &[1.0, 0.0, 0.0, 0.0],
        "bge-m3",
        "hash-semantic",
    )
    .expect("insert semantic row");
    db.upsert_wiki_vector(
        "wiki/projects/keyword-hit.md",
        &[0.0, 1.0, 0.0, 0.0],
        "bge-m3",
        "hash-keyword",
    )
    .expect("insert keyword row");

    let embeddings = Arc::new(Mutex::new(vec![vec![1.0, 0.0, 0.0, 0.0]]));
    let (base_url, handle) = start_embed_stub(embeddings).await;

    let server = make_server(tmp.path(), db, Some(&base_url));
    let json = server
        .do_wiki_search(WikiSearchParams {
            query: "git 자동화".to_string(),
            category: None,
            limit: Some(5),
            mode: Some(WikiSearchMode::Hybrid),
        })
        .expect("hybrid search");

    handle.abort();

    let paths: Vec<String> = json["results"]
        .as_array()
        .expect("results array")
        .iter()
        .filter_map(|item| item["path"].as_str().map(ToString::to_string))
        .collect();
    assert!(paths
        .iter()
        .any(|path| path == "wiki/projects/keyword-hit.md"));
    assert!(paths
        .iter()
        .any(|path| path == "wiki/projects/semantic-hit.md"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_semantic_fallback_on_embed_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_page(
        &tmp,
        "wiki/projects/fallback.md",
        "# Fallback\n\ngit 자동화 fallback keyword result",
    );

    // 도달 불가한 임베더 → embed 실패 → do_wiki_search 가 keyword 로 폴백.
    let server = make_server(
        tmp.path(),
        Database::open_memory().expect("db"),
        Some("http://127.0.0.1:9"),
    );
    let json = server
        .do_wiki_search(WikiSearchParams {
            query: "fallback".to_string(),
            category: None,
            limit: Some(5),
            mode: Some(WikiSearchMode::Semantic),
        })
        .expect("fallback search");

    assert_eq!(json["count"], 1);
    assert_eq!(json["results"][0]["path"], "wiki/projects/fallback.md");
}
