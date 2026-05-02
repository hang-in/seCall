use std::sync::Arc;

use anyhow::Result;
use secall_core::{
    jobs::{BroadcastSink, CommandAdapters, JobExecutor},
    mcp::rest::start_rest_server,
    search::tokenizer::create_tokenizer,
    search::vector::create_vector_indexer,
    search::{Bm25Indexer, SearchEngine},
    store::{get_default_db_path, Database},
    vault::Config,
};

pub async fn run(port: u16) -> Result<()> {
    let db_path = get_default_db_path();
    let db = Database::open(&db_path)?;

    // 시작 시 running/started → interrupted 보정 (서버 재시작 등으로 남은 in-flight job 처리)
    db.conn().execute(
        "UPDATE jobs SET status = 'interrupted', completed_at = datetime('now') \
         WHERE status IN ('started', 'running')",
        [],
    )?;
    let cleaned = db.cleanup_old_jobs()?;
    if cleaned > 0 {
        tracing::info!("Cleaned up {} old jobs", cleaned);
    }

    let db_arc = Arc::new(std::sync::Mutex::new(db));

    // 명령 어댑터: REST 핸들러가 args/sink만 넘기면 실제 실행은 secall crate 측에서.
    //
    // `run_with_progress`는 내부에서 `Database`(rusqlite Connection, !Sync) 참조를 await
    // 너머로 들고 다니므로 그대로는 `Send` future가 안 된다. 이를 `spawn_blocking` +
    // current-thread runtime으로 격리하면 spawn_blocking이 반환하는 JoinHandle은 Send이고,
    // 어댑터의 외부 await도 Send safe해진다. 대안으로 LocalSet도 가능하지만 axum/tokio
    // 멀티스레드 런타임에서는 spawn_blocking이 더 단순하다.
    let cmd_adapters = CommandAdapters {
        sync_fn: Box::new(|val, sink: BroadcastSink| {
            Box::pin(async move {
                tokio::task::spawn_blocking(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(async move {
                        let args: crate::commands::sync::SyncArgs = serde_json::from_value(val)?;
                        let outcome = crate::commands::sync::run_with_progress(args, &sink).await?;
                        Ok::<_, anyhow::Error>(serde_json::to_value(outcome)?)
                    })
                })
                .await?
            })
        }),
        ingest_fn: Box::new(|val, sink: BroadcastSink| {
            Box::pin(async move {
                tokio::task::spawn_blocking(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(async move {
                        let args: crate::commands::ingest::IngestArgs =
                            serde_json::from_value(val)?;
                        let outcome =
                            crate::commands::ingest::run_with_progress(args, &sink).await?;
                        Ok::<_, anyhow::Error>(serde_json::to_value(outcome)?)
                    })
                })
                .await?
            })
        }),
        wiki_update_fn: Box::new(|val, sink: BroadcastSink| {
            Box::pin(async move {
                tokio::task::spawn_blocking(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    rt.block_on(async move {
                        let args: crate::commands::wiki::WikiUpdateArgs =
                            serde_json::from_value(val)?;
                        let outcome = crate::commands::wiki::run_with_progress(args, &sink).await?;
                        Ok::<_, anyhow::Error>(serde_json::to_value(outcome)?)
                    })
                })
                .await?
            })
        }),
    };

    let executor = Arc::new(JobExecutor::with_adapters(db_arc.clone(), cmd_adapters));

    let config = Config::load_or_default();
    let tok = create_tokenizer(&config.search.tokenizer)
        .map_err(|e| anyhow::anyhow!("tokenizer init failed: {e}"))?;
    let bm25 = Bm25Indexer::new(tok);
    let vector = create_vector_indexer(&config).await;
    let search = SearchEngine::new(bm25, vector);
    let vault_path = config.vault.path.clone();

    start_rest_server(db_arc, search, vault_path, port, executor).await
}
