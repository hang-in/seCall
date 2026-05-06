---
type: task
plan_slug: p40-wiki-bm25-hybrid
task_id: 04
title: CLI backfill — `secall wiki vectorize`
parallel_group: C
depends_on: [02]
status: pending
updated_at: 2026-05-06
---

# Task 04 — `secall wiki vectorize` CLI 명령 (19 페이지 backfill)

## Changed files

수정:

- `crates/secall/src/commands/wiki.rs` — 기존 `Wiki` subcommand enum 에 `Vectorize` variant 추가 (없으면 신규 추가). 기존 wiki 생성 명령 (update 등) 과 같은 enum.
- `crates/secall/src/main.rs` 또는 dispatcher — `Vectorize` variant 의 핸들러 wiring (`Wiki::Vectorize { force } => commands::wiki::vectorize(...).await`)

신규 (선택, 코드량 ~80 LOC 예상이라 wiki.rs 안에 inline 도 가능):

- `crates/secall/src/commands/wiki.rs` 안에 `pub async fn vectorize(args: VectorizeArgs) -> anyhow::Result<()>` 함수 추가

## Change description

### 1. CLI 인자

```rust
#[derive(clap::Args)]
pub struct VectorizeArgs {
    /// content_hash 무시하고 모든 페이지 재인덱싱.
    #[arg(long)]
    pub force: bool,

    /// 모델 ID 명시 (default: bge-m3).
    #[arg(long, default_value = "bge-m3")]
    pub model: String,

    /// Ollama base URL (default: http://localhost:11434).
    #[arg(long, env = "OLLAMA_BASE_URL", default_value = "http://localhost:11434")]
    pub ollama_url: String,
}
```

### 2. 동작

```rust
pub async fn vectorize(args: VectorizeArgs) -> anyhow::Result<()> {
    let config = load_config()?;
    let db = open_db(&config)?;
    let embedder = OllamaEmbedder::new(Some(&args.ollama_url), Some(&args.model));

    let indexer = WikiIndexer {
        vault_path: &config.vault.path,
        db: &db,
        embedder: &embedder,
        model_id: &args.model,
    };

    println!("Scanning wiki pages under: {}", config.vault.path.display());
    let result = if args.force {
        indexer.reindex_all().await?  // task 02 가 force 플래그 미제공 시 본 task 에서 추가
    } else {
        indexer.index_all().await?
    };

    println!(
        "Wiki vectorize complete: scanned={} indexed={} skipped={} deleted={} failed={}",
        result.scanned,
        result.indexed,
        result.skipped,
        result.deleted,
        result.failed.len()
    );
    for (path, err) in &result.failed {
        eprintln!("  FAIL {path}: {err}");
    }
    if !result.failed.is_empty() {
        anyhow::bail!("{} pages failed to index", result.failed.len());
    }
    Ok(())
}
```

### 3. progress 출력

페이지 19 → 진행률 출력 단순화 (각 페이지 처리 시 stdout 1줄: `[i/19] indexed wiki/projects/X.md (320 ms)` 또는 indexer 가 callback 받는 형태). 본 plan 에서는 간단히 sync log + 최종 요약 line 1줄. 100+ 도래 시 progress bar 도입 별도 phase.

### 4. idempotent 보장

- 기본 (no `--force`): content_hash 일치 = skip
- `--force`: hash 무시, 전 페이지 재인덱싱 (모델 변경 시 활용)

## Dependencies

- **Task 02 필수** — `WikiIndexer` 인프라
- (선택) Task 03 — 검색 검증을 위해 본 task 후 `secall recall --mode hybrid` 또는 `/api/wiki?mode=hybrid` 로 backfill 결과 확인 가능
- 외부: Ollama 가 `localhost:11434` 에서 실행 중 + `bge-m3` 모델 로드됨 (P22 wiki 파이프라인이 이미 같은 stack 사용 → 사용자 환경 OK 가정)

## Verification

```bash
# 1. 컴파일
cargo build -p secall --release

# 2. CLI help — 명령 등록 확인
./target/release/secall wiki vectorize --help

# 3. (수동 / 통합) 19 페이지 backfill 실제 실행
./target/release/secall wiki vectorize
# 기대: "Wiki vectorize complete: scanned=19 indexed=19 skipped=0 deleted=0 failed=0"

# 4. (수동) idempotent 검증 — 다시 실행 시 모두 skip
./target/release/secall wiki vectorize
# 기대: "scanned=19 indexed=0 skipped=19 deleted=0 failed=0"

# 5. (수동) DB 검증
sqlite3 ~/Library/Caches/secall/index.sqlite "SELECT COUNT(*) FROM wiki_vectors"
# 기대: 19

# 6. (수동) hybrid 검색 동작 확인 (task 03 완료 전제)
curl -s -X POST http://localhost:3000/api/wiki \
  -H "content-type: application/json" \
  -d '{"query":"git 자동화","mode":"hybrid","limit":5}' | jq '.count, .results[].path'
# 기대: count > 0, vault auto_commit 관련 페이지 hit
```

## Risks

- **Ollama 미실행 / `bge-m3` 미로드**: backfill 명령 실행 실패 → 명령이 명확한 에러 메시지로 종료. 사용자가 `ollama pull bge-m3` 후 재시도.
- **DB lock**: secall 다른 명령 (sync 등) 동시 실행 시 SQLite lock. 메시지로 안내. 단일 사용자 환경이라 실용 빈도 낮음.
- **`reindex_all` 메서드**: task 02 가 `index_all` 만 제공. 본 task 가 `--force` 를 위해 추가 필요 → task 02 문서 수정 또는 본 task 에서 indexer 의 동작 토글 (e.g., `index_all_with(force: bool)`).

## Scope boundary (수정 금지)

- `crates/secall/src/commands/sync.rs` — sync 명령 (별도 영역)
- `crates/secall/src/commands/ingest.rs` — ingest 명령
- `crates/secall-core/` 의 모든 파일 — task 01~03 영역. 본 task 는 CLI thin wrapper 만 추가.
