# Review Report: seCall Refactor P4 — 아키텍처 개선 — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-06 17:15
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/vector.rs:334 — Task 02 계약과 달리 `search/vector.rs`에 `impl Database` 블록이 여전히 남아 있습니다. task의 검증 조건(`grep -n "^impl Database" ...`)도 이 상태에서는 충족되지 않습니다.
2. crates/secall-core/src/search/hybrid.rs:142 — `vec_only` CLI 경로는 `SearchEngine::search_vector()`를 타고, 그 구현이 `VectorIndexer::search()`의 선형 `db.search_vectors(...)` 경로를 호출합니다. 새 ANN 검색은 `search_with_embedding()`에만 연결되어 있어 `--vec` 전용 최적화가 실제로 동작하지 않습니다.
3. crates/secall-core/src/search/vector.rs:73 — ANN 인덱스에 벡터를 추가한 뒤 운영 경로에서 `AnnIndex::save()`를 호출하지 않아, 프로세스 재시작 시 `.usearch` 파일이 stale 상태로 남습니다. 결과적으로 새로 인덱싱한 벡터가 이후 ANN 검색에서 누락될 수 있습니다.

## Recommendations

1. `SearchEngine::search_vector()`가 `search_with_embedding()`와 동일한 ANN-aware 경로를 사용하도록 공통화하세요.
2. ANN 인덱스 변경 후 저장 시점을 명시적으로 두세요. 최소한 `embed --all` 완료 시점 또는 세션 인덱싱 배치 종료 시점에 `save()`가 호출되어야 합니다.
3. `get_vector_meta`를 `VectorRepo` 또는 `store/db.rs`로 이동해 Task 02의 repository 경계를 일관되게 맞추세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | typed error 도입 (SecallError enum) | ✅ done |
| 2 | Database Repository 패턴 | ✅ done |
| 3 | ANN 인덱스 도입 (--vec 전용) | ✅ done |

