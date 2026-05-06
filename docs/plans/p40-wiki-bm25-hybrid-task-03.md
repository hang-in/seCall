---
type: task
plan_slug: p40-wiki-bm25-hybrid
task_id: 03
title: Search hybrid mode — do_wiki_search 확장
parallel_group: B
depends_on: [01]
status: pending
updated_at: 2026-05-06
---

# Task 03 — `do_wiki_search` 에 semantic + hybrid mode 추가

## Changed files

수정:

- `crates/secall-core/src/mcp/tools.rs` — `WikiSearchParams` 구조체에 `mode: Option<WikiSearchMode>` 필드 추가 + `WikiSearchMode` enum 정의 (`Keyword`, `Semantic`, `Hybrid`). 직렬화 lower-case (`#[serde(rename_all = "lowercase")]`).
- `crates/secall-core/src/mcp/server.rs:297` — `do_wiki_search` 시그니처 유지, 함수 본체 분기 추가:
  - `mode == None | Some(Keyword)` → 기존 substring 매칭 (현재 동작 그대로)
  - `mode == Some(Semantic)` → `wiki_vectors` 코사인 유사도 기반 top-k
  - `mode == Some(Hybrid)` → keyword + semantic 결과를 RRF 결합
  - semantic/hybrid 실패 (Ollama 등) 시 `tracing::warn!` + keyword fallback
- `crates/secall-core/src/mcp/server.rs` 에 private 헬퍼 추가:
  - `fn do_wiki_search_semantic(&self, params: &WikiSearchParams) -> Result<Vec<Match>>`
  - `fn do_wiki_search_hybrid(&self, ...)` — 두 결과 RRF 결합
- `crates/secall-core/src/mcp/server.rs:332` (Match struct) — score 필드 추가 또는 별도 ranked struct 도입 (RRF score 보존)
- `crates/secall-core/tests/wiki_search_modes.rs` (신규 통합 테스트) — keyword/semantic/hybrid 각 1건 + fallback 1건 = 4 tests

## Change description

### 1. WikiSearchParams 확장 (tools.rs)

```rust
#[derive(Debug, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum WikiSearchMode {
    #[default]
    Keyword,
    Semantic,
    Hybrid,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WikiSearchParams {
    pub query: String,
    pub category: Option<String>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub mode: Option<WikiSearchMode>,  // None == Keyword (호환)
}
```

> 호환성 핵심: `mode` 미지정 시 = `Keyword` = 기존 동작. 기존 호출자 (REST/MCP/web) 변경 없음.

### 2. do_wiki_search 분기 (server.rs:297)

```rust
pub fn do_wiki_search(&self, params: WikiSearchParams) -> anyhow::Result<serde_json::Value> {
    let mode = params.mode.unwrap_or_default();
    match mode {
        WikiSearchMode::Keyword => self.do_wiki_search_keyword(&params),
        WikiSearchMode::Semantic => self
            .do_wiki_search_semantic(&params)
            .or_else(|e| {
                tracing::warn!(error=%e, "semantic search failed, falling back to keyword");
                self.do_wiki_search_keyword(&params)
            }),
        WikiSearchMode::Hybrid => self
            .do_wiki_search_hybrid(&params)
            .or_else(|e| {
                tracing::warn!(error=%e, "hybrid search failed, falling back to keyword");
                self.do_wiki_search_keyword(&params)
            }),
    }
}
```

기존 line 297–408 의 코드는 `do_wiki_search_keyword` 로 추출 (동작 동일).

### 3. semantic 구현 (server.rs 내 신규 메서드)

1. `Embedder` (이미 server 가 가지고 있는지 확인 — 없으면 server 필드 추가, 또는 lazy init). `Server` 에 embedder 가 없으면 `OllamaEmbedder::new(default)` lazy 생성.
2. query → `embedder.embed(&params.query).await?` (참고: 본 함수는 비동기여야 — 시그니처 변경 필요. server 의 다른 비동기 함수 패턴 따라 `async fn` 으로 전환. 호출자 (`api_wiki`) 는 이미 axum async handler 라 영향 X).
3. `db.list_wiki_vectors()` → 각 row 의 embedding 과 query embedding cosine 유사도 계산 → top-k.
4. category 필터 적용 (path 가 `wiki/{cat}/` 로 시작하는지).
5. fs 에서 본문 읽어 preview/title 추출 (기존 keyword 와 동일 포맷).

### 4. hybrid 구현

1. `do_wiki_search_keyword(params)` 호출 → 결과 N개
2. `do_wiki_search_semantic(params)` 호출 → 결과 M개
3. RRF: 각 결과 set 에서 path 별 rank → `score(path) = sum(1/(k+rank))`, k=60 (recall 패턴)
4. 통합 score desc 정렬 → top-`limit`
5. recall 의 `crates/secall-core/src/search/hybrid.rs` 의 RRF 로직을 **참고** (재사용은 시그니처 호환되면, 아니면 본 함수 내 inline ~10 LOC)

### 5. 통합 테스트 (tests/wiki_search_modes.rs)

- `test_keyword_mode_default_when_mode_none` — 기존 호환
- `test_semantic_mode_returns_results` — mock Embedder + tempvault
- `test_hybrid_mode_combines_both` — keyword 만 hit + semantic 만 hit 인 두 페이지 → 둘 다 결과 포함
- `test_semantic_fallback_on_embed_failure` — embed 가 Err 반환 → keyword 결과 반환

## Dependencies

- **Task 01 필수** — `wiki_vectors` 테이블 + `WikiVectorRepo`
- **Task 02 권장** — 인덱서가 만들어 둔 row 가 있어야 semantic 결과 의미. 단 unit test 는 직접 row insert 로 우회 가능 → 02 와 병렬 진행 가능.
- 기존 `Embedder` trait + `OllamaEmbedder`

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core

# 2. 신규 통합 테스트 4건
cargo test -p secall-core --test wiki_search_modes

# 3. 기존 do_wiki_search 회귀 (server.rs::tests)
cargo test -p secall-core --lib mcp::server::tests

# 4. REST 회귀 — /api/wiki 가 mode 미지정 시 기존 응답 형태 유지
cargo test -p secall-core --test rest_routes test_wiki
```

## Risks

- **`do_wiki_search` async 전환**: 호출자 (REST `api_wiki`, MCP `wiki_search` tool) 가 이미 async context → 안전. 단 cargo check 에서 trait bound 변경으로 인한 컴파일 에러 가능 → 신중히 추적.
- **server 에 Embedder 추가**: 기존 `Server` struct 에 embedder 필드 없을 가능성. 새 필드 추가 시 생성자 변경 → 호출 사이트 전수 점검 필요.
- **WikiSearchMode JsonSchema**: rmcp 는 `schemars` 매크로 사용. enum 직렬화 derive 가 deserialize 에 호환되는지 검증.
- **RRF k=60 magic**: recall 의 hybrid 가 사용하는 값과 일치. 추후 튜닝 phase 에서 조정 가능 (본 plan 영역 X).

## Scope boundary (수정 금지)

- `crates/secall-core/src/search/hybrid.rs` — RRF 로직 참고만, **시그니처 변경 X**. 만약 inline 복제로 갈 시 그 사실을 commit 메시지에 명시.
- `crates/secall-core/src/search/bm25.rs` — 본 plan 은 BM25 도입 X, 현행 substring 매칭 유지.
- `crates/secall-core/src/wiki/` 의 generation backend 들
- `crates/secall-core/src/store/db.rs` (Database 본체 — task 01 의 migrate 만)

## 호환성 검증 체크리스트

- [ ] `WikiSearchParams { query, category: None, limit: Some(5), mode: None }` 호출 → 기존 keyword 결과와 동일 JSON
- [ ] REST `/api/wiki` body `{"query":"x"}` → 동일
- [ ] MCP `wiki_search` tool 호출 → 동일
- [ ] web/`useWiki.ts` 의 호출 — DTO 추가 컬럼이 unknown field 라 무시되는지 ([서버→웹 응답 일관])
