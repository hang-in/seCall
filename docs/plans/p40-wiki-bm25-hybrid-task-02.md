---
type: task
plan_slug: p40-wiki-bm25-hybrid
task_id: 02
title: Wiki indexing — 페이지 → embedding 저장
parallel_group: B
depends_on: [01]
status: pending
updated_at: 2026-05-06
---

# Task 02 — Wiki indexing 인프라

## Changed files

신규:

- `crates/secall-core/src/store/wiki_vector_repo.rs` (신규) — `WikiVectorRepo` trait + `Database` impl. CRUD: `init_wiki_vector_table` (no-op, schema.rs 가 처리), `upsert_wiki_vector(path, embedding, model_id, content_hash)`, `get_wiki_vector(path) -> Option<row>`, `list_wiki_vectors() -> Vec<row>`, `delete_wiki_vector(path)`.
- `crates/secall-core/src/wiki/indexer.rs` (신규) — `WikiIndexer` 구조체. 책임: `vault_path/wiki/**/*.md` 스캔 → 각 페이지의 SHA-256 hash 계산 → DB content_hash 비교 → unchanged skip / 변경 시 `Embedder::embed` 호출 → `upsert_wiki_vector`.
- `crates/secall-core/tests/wiki_indexer.rs` (신규) — 회귀 테스트 (단일 페이지 인덱싱 + 동일 hash skip + 변경 시 재인덱싱).

수정:

- `crates/secall-core/src/store/mod.rs` — `pub mod wiki_vector_repo;` + re-export `WikiVectorRepo` (기존 `VectorRepo` 패턴 따라)
- `crates/secall-core/src/wiki/mod.rs` — `pub mod indexer;` + `pub use indexer::WikiIndexer;`

## Change description

### 1. `WikiVectorRepo` trait (store/wiki_vector_repo.rs)

`crates/secall-core/src/store/vector_repo.rs` 의 `VectorRepo` trait 패턴을 거의 그대로 미러링하되, 범위는 wiki_vectors 단일 테이블:

```rust
pub trait WikiVectorRepo {
    fn upsert_wiki_vector(
        &self,
        wiki_path: &str,
        embedding: &[f32],
        model_id: &str,
        content_hash: &str,
    ) -> anyhow::Result<()>;

    fn get_wiki_vector(&self, wiki_path: &str)
        -> anyhow::Result<Option<WikiVectorRow>>;

    fn list_wiki_vectors(&self) -> anyhow::Result<Vec<WikiVectorRow>>;

    fn delete_wiki_vector(&self, wiki_path: &str) -> anyhow::Result<()>;
}

pub struct WikiVectorRow {
    pub wiki_path: String,
    pub embedding: Vec<f32>,
    pub model_id: String,
    pub content_hash: String,
    pub updated_at: String,
}
```

`embedding` BLOB 직렬화는 `turn_vectors` 와 동일 방식 (f32 little-endian) — `vector_repo.rs` 에 이미 있는 변환 헬퍼가 private 이면 본 모듈에 동일 패턴 복제 (DRY 위반 의식적 — 두 영역의 결합 회피).

### 2. `WikiIndexer` (wiki/indexer.rs)

```rust
pub struct WikiIndexer<'a> {
    pub vault_path: &'a Path,
    pub db: &'a Database,
    pub embedder: &'a dyn Embedder,
    pub model_id: &'a str,  // e.g., "bge-m3"
}

pub struct IndexResult {
    pub scanned: usize,
    pub indexed: usize,    // 신규 또는 변경
    pub skipped: usize,    // 동일 content_hash
    pub deleted: usize,    // DB 에는 있으나 fs 에 없는 row 정리
    pub failed: Vec<(String, anyhow::Error)>,
}

impl<'a> WikiIndexer<'a> {
    pub async fn index_all(&self) -> anyhow::Result<IndexResult> { ... }
}
```

알고리즘:
1. `walkdir::WalkDir(vault_path/wiki)` 로 `.md` 전체 수집 → 상대경로 (vault 기준) 리스트
2. 각 파일: 본문 읽기 → `sha2::Sha256` 으로 hash → `db.get_wiki_vector(path)` 비교
3. unchanged: skip
4. 변경/신규: `embedder.embed(content).await?` → `db.upsert_wiki_vector(...)`
5. fs 에 없는 DB row → `db.delete_wiki_vector(path)`
6. 모든 단계 individual error catch → `IndexResult.failed` 수집 (한 페이지 실패가 전체 abort 시키지 않음)

### 3. 회귀 테스트 (tests/wiki_indexer.rs)

- `test_index_single_page_inserts_row`
- `test_unchanged_page_skipped`
- `test_modified_page_reindexed`
- `test_deleted_file_removes_row`

`Embedder` mock — fixed-vector returning `OllamaEmbedder` 를 직접 쓰지 않고 trait 구현체 inline (e.g., 항상 `vec![0.1; 4]` 반환).

## Dependencies

- **Task 01 필수** — `wiki_vectors` 테이블 존재 전제
- crate dep — `sha2` (이미 워크스페이스에 있음, `Cargo.toml:43`)
- 기존 `Embedder` trait (`crates/secall-core/src/search/embedding.rs:12`) — 변경 X, 사용만

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core

# 2. 회귀 테스트 (신규 4건)
cargo test -p secall-core --test wiki_indexer

# 3. 기존 store/wiki 테스트 회귀 — 변경 없어야 함
cargo test -p secall-core --lib store::
cargo test -p secall-core --lib wiki::
```

## Risks

- **bge-m3 미실행 환경에서 테스트 hang**: 테스트는 mock Embedder 사용 → 실제 Ollama 무관.
- **`f32 BLOB` 직렬화 mismatch**: turn_vectors 와 일관성 위해 동일 byte order 사용. 위반 시 search 단계 (task 03) 에서 cosine 결과 무의미.
- **content_hash 충돌**: SHA-256 → 충돌 무시 (실용 0).
- **스캔 시간**: 19 페이지면 무시 (~수 ms). 100+ 도래 시 별도 phase 에서 incremental fs watch 도입 검토 (현재 plan 외).

## Scope boundary (수정 금지)

- `crates/secall-core/src/store/vector_repo.rs` (turn_vectors VectorRepo)
- `crates/secall-core/src/search/embedding.rs` (Embedder trait + OllamaEmbedder)
- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio,review,lint}.rs` (wiki 생성 영역)
- `crates/secall-core/src/store/db.rs` (Database struct 본체 — task 01 의 migrate 만 변경, 본 task 영역 외)
