use anyhow::Result;
use secall_core::{
    store::{get_default_db_path, Database},
    vault::Config,
};

pub fn run() -> Result<()> {
    let config = Config::load_or_default();
    let db_path = get_default_db_path();

    println!("seCall Status");
    println!("=============");
    println!("Config: {}", Config::config_path().display());
    println!("DB:     {}", db_path.display());
    println!("Vault:  {}", config.vault.path.display());
    println!();

    println!("Settings:");
    println!("  tokenizer  = {}", config.search.tokenizer);

    let embedding_detail = match config.embedding.backend.as_str() {
        "ollama" => {
            let model = config
                .embedding
                .ollama_model
                .as_deref()
                .unwrap_or("qwen3-embedding:0.6b");
            format!("ollama ({})", model)
        }
        "ort" => "ort (local ONNX, CPU)".to_string(),
        "openvino" => {
            let device = config.embedding.openvino_device.as_deref().unwrap_or("GPU");
            format!("openvino ({device})")
        }
        "openai" => {
            let model = config
                .embedding
                .openai_model
                .as_deref()
                .unwrap_or("text-embedding-3-large");
            format!("openai ({})", model)
        }
        "none" => "none (벡터 검색 비활성화)".to_string(),
        other => other.to_string(),
    };
    println!("  embedding  = {}", embedding_detail);

    if config.vault.git_remote.is_some() {
        println!("  branch     = {}", config.vault.branch);
    }
    println!("  timezone   = {}", config.output.timezone);
    println!();

    if !db_path.exists() {
        println!("DB does not exist. Run `secall init` first.");
        return Ok(());
    }

    let db = Database::open(&db_path)?;

    let stats = db.get_stats()?;
    println!("Index Statistics:");
    println!("  Sessions:      {}", stats.session_count);
    println!("  Turns:         {}", stats.turn_count);
    println!("  Embedded:      {}", stats.vector_count);
    // 임베딩 모델 불일치 경고 — 교차모델 벡터는 차원이 같아도 유사도가 무의미해
    // 시맨틱 검색이 조용히 저하된다.
    let embed_identity = config.embedding.embedding_identity();
    match db.get_embedding_model() {
        Ok(Some(stored)) if stored != embed_identity => {
            println!("  ⚠ 임베딩 모델 불일치: 기존 벡터={stored} / 현재={embed_identity}");
            println!("     → `secall embed --all` 로 재임베딩 권장 (BM25 키워드 검색은 무관)");
        }
        Ok(None) if stats.vector_count > 0 => {
            println!(
                "  ⚠ 임베딩 모델 마커 없음 (v0.7.0 이전 DB 일 수 있음). 벡터가 현재 모델({embed_identity})과 다르면 `secall embed --all` 권장"
            );
        }
        _ => {}
    }
    println!();

    // Vault status
    let sessions_dir = secall_core::vault::sessions_subdir(&config.vault.path);
    if sessions_dir.exists() {
        let md_count = walkdir::WalkDir::new(&sessions_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
            .count();
        println!("Vault Files:     {} session markdown files", md_count);
    } else {
        println!("Vault: not initialized");
    }

    // Recent ingest log
    println!("\nRecent Ingests (last 5):");
    for entry in &stats.recent_ingests {
        println!(
            "  {} — {} {}",
            entry.timestamp, entry.agent, entry.session_id_prefix
        );
    }

    Ok(())
}
