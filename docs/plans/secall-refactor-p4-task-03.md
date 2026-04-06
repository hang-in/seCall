---
type: task
status: draft
plan: secall-refactor-p4
task_number: 3
title: "ANN 인덱스 도입 (--vec 전용)"
parallel_group: B
depends_on: [1]
updated_at: 2026-04-06
---

# Task 03: ANN 인덱스 도입 (--vec 전용)

## 문제

`vector.rs:342-350`의 `search_vectors()`가 `session_ids: None`일 때 전체 테이블을 메모리에 로드하여 선형 스캔한다:

```sql
-- vector.rs:343 (session_ids: None 경로)
SELECT id, session_id, turn_index, chunk_seq, embedding FROM turn_vectors
-- WHERE 없음
```

P2에서 하이브리드 검색(RRF) 경로는 `session_ids` 필터로 해소되었으나, `--vec` 전용 모드에서는 여전히 O(n). 벡터 차원은 embedder별 동적 (Ollama/ORT: 1024, OpenAI: 1536/3072).

개인 사용이라도 1년 이상 누적 시 수만 청크에 도달하며 검색 지연이 급증한다.

## Changed files

| 파일 | 변경 | 비고 |
|---|---|---|
| `Cargo.toml` (workspace) | 수정 | `usearch` 의존성 추가 |
| `crates/secall-core/Cargo.toml` | 수정 | `usearch.workspace = true` |
| `crates/secall-core/src/search/ann.rs` | 신규 | ANN 인덱스 래퍼 (빌드/로드/검색/저장) |
| `crates/secall-core/src/search/vector.rs` | 수정 | `VectorIndexer`에 ANN 인덱스 통합 |
| `crates/secall-core/src/search/mod.rs` | 수정 | `pub mod ann;` 추가 |

## Change description

### Step 1: usearch 의존성 추가

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
usearch = "2"

# crates/secall-core/Cargo.toml
[dependencies]
usearch.workspace = true
```

> `usearch` crate: C++로 구현된 HNSW 기반 ANN 라이브러리의 Rust 바인딩. 파일 기반 인덱스 저장/로드 지원.

### Step 2: ANN 인덱스 래퍼 (ann.rs — 신규)

```rust
// crates/secall-core/src/search/ann.rs
use anyhow::Result;
use std::path::{Path, PathBuf};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

pub struct AnnIndex {
    index: Index,
    path: PathBuf,
    dimensions: usize,
}

impl AnnIndex {
    /// 기존 인덱스 파일이 있으면 로드, 없으면 새로 생성.
    pub fn open_or_create(path: &Path, dimensions: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions,
            metric: MetricKind::Cos,   // cosine similarity
            quantization: ScalarKind::F32,
            ..Default::default()
        };
        let index = Index::new(&options)?;

        if path.exists() {
            index.load(path)?;
            tracing::info!(path = %path.display(), vectors = index.size(), "ANN index loaded");
        } else {
            // 빈 인덱스 — 벡터 삽입 시 자동 확장
            index.reserve(10_000)?;
            tracing::info!(path = %path.display(), "ANN index created (empty)");
        }

        Ok(Self {
            index,
            path: path.to_path_buf(),
            dimensions,
        })
    }

    /// 벡터 추가. key는 turn_vectors 테이블의 rowid.
    pub fn add(&self, key: u64, vector: &[f32]) -> Result<()> {
        self.index.add(key, vector)?;
        Ok(())
    }

    /// ANN 검색. 상위 limit개의 (key, distance) 반환.
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(u64, f32)>> {
        let results = self.index.search(query, limit)?;
        Ok(results.keys.into_iter().zip(results.distances).collect())
    }

    /// 인덱스를 파일에 저장.
    pub fn save(&self) -> Result<()> {
        self.index.save(&self.path)?;
        tracing::info!(path = %self.path.display(), vectors = self.index.size(), "ANN index saved");
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.index.size()
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}
```

### Step 3: VectorIndexer에 ANN 통합 (vector.rs)

```rust
// vector.rs:26-28 — 변경 전
pub struct VectorIndexer {
    embedder: Box<dyn Embedder>,
}

// 변경 후
pub struct VectorIndexer {
    embedder: Box<dyn Embedder>,
    ann_index: Option<AnnIndex>,  // None이면 기존 BLOB 스캔 fallback
}
```

### Step 4: index_session()에서 ANN 인덱스에도 추가

```rust
// vector.rs — index_session() 내부, insert_vector 성공 후
if let Some(ref ann) = self.ann_index {
    if let Err(e) = ann.add(rowid as u64, &embedding) {
        tracing::warn!(error = %e, "ANN index add failed");
    }
}
```

### Step 5: search_vectors()의 None 경로를 ANN으로 대체

```rust
// vector.rs — search 메서드 (VectorIndexer::search 또는 search_with_embedding)
// session_ids: None이고 ANN 인덱스 사용 가능할 때

if candidate_session_ids.is_none() {
    if let Some(ref ann) = self.ann_index {
        // ANN 검색 → rowid로 DB에서 메타데이터 조회
        let ann_results = ann.search(query_embedding, limit)?;
        let rows: Vec<VectorRow> = ann_results.iter()
            .filter_map(|(key, distance)| {
                // rowid로 turn_vectors에서 session_id, turn_index, chunk_seq 조회
                db.get_vector_meta(*key as i64).ok()
                    .map(|(session_id, turn_index, chunk_seq)| VectorRow {
                        rowid: *key as i64,
                        distance: *distance,
                        session_id,
                        turn_index,
                        chunk_seq,
                    })
            })
            .collect();
        return Ok(rows);
    }
}
// ... 기존 BLOB 스캔 fallback ...
```

### Step 6: Database에 벡터 메타 조회 헬퍼 추가

```rust
// vector.rs — impl VectorRepo for Database (또는 impl Database)
pub fn get_vector_meta(&self, rowid: i64) -> Result<(String, u32, u32)> {
    self.conn().query_row(
        "SELECT session_id, turn_index, chunk_seq FROM turn_vectors WHERE id = ?1",
        [rowid],
        |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u32, row.get::<_, i64>(2)? as u32)),
    ).map_err(|e| e.into())
}
```

### Step 7: ANN 인덱스 파일 경로

```
~/.config/secall/ann_index.usearch
```

`create_vector_indexer()`에서 ANN 인덱스도 함께 로드:

```rust
// vector.rs — create_vector_indexer() 수정
let ann_path = dirs::config_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("secall")
    .join("ann_index.usearch");

let ann_index = match AnnIndex::open_or_create(&ann_path, embedder.dimensions()) {
    Ok(idx) => Some(idx),
    Err(e) => {
        tracing::warn!(error = %e, "ANN index unavailable, falling back to BLOB scan");
        None
    }
};
```

### Step 8: embed 명령에서 ANN 인덱스 리빌드

`secall embed --all` 실행 시 기존 BLOB에서 ANN 인덱스를 리빌드하는 옵션 추가:

```rust
// embed.rs — --rebuild-ann 플래그 또는 embed 종료 시 자동 저장
if let Some(ref ann) = vector_indexer.ann_index {
    ann.save()?;
}
```

## Dependencies

- **Task 01 (typed error)**: `Result` 타입이 확정되어야 함
- `usearch` crate — 신규 의존성 (외부)
- 기존 `ort`, `rusqlite` 의존성은 변경 없음

## Verification

```bash
# 1. 컴파일 확인 (usearch C++ 빌드 포함)
cargo check --all

# 2. 벡터 테스트 통과
cargo test -p secall-core vector

# 3. ANN 테스트 (신규)
cargo test -p secall-core ann

# 4. 전체 테스트 회귀 없음
cargo test --all

# 5. ANN 없이도 기존 동작 유지 (graceful fallback)
# Manual: ANN 인덱스 파일 삭제 후 `secall recall "test" --vec` 실행 — BLOB 스캔으로 정상 동작 확인

# 6. clippy 통과
cargo clippy --all-targets -- -D warnings
```

> **[Developer 필수]** subtask-done 시그널 전에 위 명령의 실행 결과를 result 문서에 기록하세요.

## Risks

- **usearch C++ 빌드**: usearch는 C++ 바인딩을 포함. CI(ubuntu)와 macOS arm64 모두에서 빌드 가능한지 확인 필요. cmake + C++ 컴파일러가 필요할 수 있음.
- **인덱스 파일 동기화**: SQLite의 turn_vectors 테이블과 ANN 인덱스 파일이 별도 관리되므로, 데이터 삭제 시 불일치 가능. 1차에서는 `embed --all`로 리빌드하는 방식으로 해결.
- **차원 불일치**: embedder 변경 시 ANN 인덱스의 차원이 달라질 수 있다. `open_or_create()`에서 기존 인덱스의 차원과 현재 embedder 차원을 비교, 불일치 시 인덱스 재생성.
- **메모리 사용량**: usearch 인덱스는 메모리 매핑 기반. 100k 벡터 × 1024차원 ≈ ~400MB 인덱스 파일이나, 메모리 매핑으로 실제 RSS는 훨씬 낮음.
- **graceful degradation**: ANN 로드 실패 시 기존 BLOB 스캔으로 fallback. `ann_index: Option<AnnIndex>`로 항상 안전.

## Scope boundary

다음 파일은 이 task에서 수정하지 않음:
- `crates/secall-core/src/search/hybrid.rs` — 하이브리드 검색은 이미 session_ids 필터 사용. ANN은 VectorIndexer 내부에서 투명하게 동작.
- `crates/secall-core/src/search/bm25.rs` — BM25 로직 변경 없음
- `crates/secall-core/src/store/schema.rs` — SQLite DDL 변경 없음 (ANN은 별도 파일)
- `crates/secall-core/src/search/chunker.rs` — 청킹 로직 변경 없음
