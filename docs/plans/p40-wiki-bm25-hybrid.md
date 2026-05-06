---
type: plan
status: in_progress
updated_at: 2026-05-06
slug: p40-wiki-bm25-hybrid
---

# P40 — Wiki 벡터화 (keyword → hybrid)

> 외부 컨트리뷰터 (CoLuthien) 의 wiki search BM25→vector hybrid 요청 + P39 baseline 측정 (19 pages, bimodal) 기반.
> 결정 문서: `docs/baseline/p39-p40-decision.md` ("P40 즉시 진행, 단순 스코프").

## Description

`crates/secall-core/src/mcp/server.rs:297` 의 `do_wiki_search` 는 현재 **filesystem substring 매칭** (BM25 인덱싱조차 없음). 아래 두 모드를 추가:

- **semantic** — bge-m3 (Ollama) 페이지 임베딩 + cosine 유사도
- **hybrid** — 기존 keyword + semantic 의 RRF 결합 (recall 의 `search/hybrid.rs` 패턴 재사용)

기존 keyword 모드는 default 유지 (호환). 페이지 19개 small scope 라 **chunker 분리 X — 페이지 단위 1 chunk = 1 embedding**.

## Expected Outcome

- `WikiSearchParams.mode = {keyword(default) | semantic | hybrid}` 동작
- `wiki_vectors` 테이블 19 페이지 backfill 완료 (1회성 CLI)
- semantic 검색 예: "git 자동화" → "vault auto_commit" 페이지 hit (현재 keyword 는 fail)
- 기존 keyword-only 호출자 (REST `/api/wiki`, MCP `wiki_search` tool, web `useWiki`) 코드 변경 없이 default 동작 유지
- 외부 컨트리뷰터 회신 게시

## Subtasks

| # | Title | parallel_group | depends_on |
|---|---|---|---|
| 01 | DB v9 마이그레이션 — `wiki_vectors` 테이블 | A | — |
| 02 | Wiki indexing — 페이지 → embedding 저장 | B | 01 |
| 03 | Search hybrid mode — `do_wiki_search` 확장 | B | 01 |
| 04 | CLI backfill — `secall wiki vectorize` | C | 02 |
| 05 | 외부 컨트리뷰터 회신 게시 (manual) | D | 03, 04 |

> Task 02 와 03 은 동일 그룹이지만 직렬 (02 가 인덱싱 인프라 제공, 03 이 그것을 검색에서 사용). 병렬 가능 시점은 04 (CLI) 와 05 (manual) 뿐.

## Constraints

- 단순 스코프 — 페이지 단위 임베딩만, chunker 분리 X
- 기존 호출자 호환 — `mode` 미지정 시 keyword 동작 유지
- 재인덱싱 cost 최소 — content_hash 기반 incremental, 19 페이지 backfill = bge-m3 batch 1회 (~몇 초)
- bge-m3 (Ollama) 그대로 사용 — backend 추가 X
- semantic 실패 (Ollama down 등) 시 keyword fallback + warn log

## Non-goals

- 섹션/문단 단위 chunking — 페이지 100+ 또는 평균 8000+ tokens 도래 후 별도 phase
- BM25 자체 도입 — 현재 substring 매칭 유지 (nontrivial 변경, 외부 컨트리뷰터의 "BM25→hybrid" 표현은 keyword 의미로 해석)
- semantic graph 와 통합
- wiki_search 호출 카운터 — `docs/baseline/p39-p40-decision.md` 의 (선택) 항목, 별도 phase
- LLM re-ranking
- 다국어 임베딩 모델 평가/교체
- 재인덱싱 자동 트리거 (sync 후 hook 등) — 1회 backfill + 수동 명령으로 충분

## Risks

| Risk | Mitigation |
|---|---|
| DB v9 마이그레이션 실패 (단일 사용자라도) | `if current < 9` 블록은 `CREATE TABLE IF NOT EXISTS` 로 idempotent. 회귀 테스트 1건 (v8→v9). |
| bge-m3 차원/모델 ID 변경 → 차원 불일치 | `wiki_vectors.model_id` 컬럼으로 감지. 미스매치 시 backfill 명령에서 warn + skip 또는 재인덱싱. |
| semantic 모드 timeout (Ollama 미실행) | `do_wiki_search` 내부에서 catch → keyword fallback + tracing::warn. |
| 페이지 19개 부족으로 semantic 효과 미미 | 결정 문서 (`docs/baseline/p39-p40-decision.md` §4) 에서 strategic 결정으로 기재. 본 plan 의 risk 가 아닌 기 합의. |

## Scope boundary (수정 금지)

- `crates/secall-core/src/search/hybrid.rs` — recall 영역, RRF 헬퍼는 **재사용만** (시그니처 변경 X). 필요 시 헬퍼를 pub 으로 노출하거나 본 plan 에서 별도 wiki RRF 작성.
- `crates/secall-core/src/store/vector_repo.rs` — `turn_vectors` 영역. 본 plan 은 새로운 `wiki_vector_repo` 모듈 추가, 기존 trait 변경 X.
- `crates/secall-core/src/semantic/` — semantic graph, 무관.
- `crates/secall-core/src/ingest/` — session ingest 파이프라인, 무관.
- `crates/secall-core/src/wiki/` 의 `claude.rs`/`codex.rs`/`haiku.rs`/`ollama.rs`/`lmstudio.rs`/`review.rs` — wiki 생성 backend, 본 plan 은 검색 영역.

## Verification (전체)

각 task 의 Verification 외, 통합 검증:

```bash
# 마이그레이션 + 인덱싱 + 검색 end-to-end (본 plan 의 task-04 가 주된 통합 명령)
cargo build -p secall --release
secall wiki vectorize        # backfill 19 페이지
secall recall --mode hybrid "git 자동화"  # 비교 baseline (recall 은 이미 hybrid)
curl -s http://localhost:3000/api/wiki -X POST \
  -d '{"query":"git 자동화","mode":"hybrid","limit":5}' | jq '.count'
```

## Status

- **2026-05-06**: P39 머지 후 promote → drafting.
