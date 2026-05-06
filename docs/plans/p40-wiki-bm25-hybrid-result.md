# Implementation Result: P40 — Wiki 벡터화 (BM25 → hybrid)

> Developer: codex
> Branch: N/A
> Date: 2026-05-06 11:40
> Plan Revision: 0

---

## Summary

Developer Codex · Implementer입니다. Rework는 Task 02 범위만 수정합니다. 지적된 `wiki_vector_repo.rs`의 무음 에러 폐기 경로를 먼저 확인하고, Task 02 검증 명령만 다시 실행하겠습니다.수정 지점은 확인했습니다. `list_wiki_vectors()`의 `query_map` 결과를 순회하면서 역직렬화 에러를 경고로 남기고 해당 row만 건너뛰도록 바꾸겠습니다.패치는 끝났습니다. 이제 Task 02 문서의 검증 명령만 다시 돌려서 rework 결과를 확인하겠습니다.Developer Codex · Implementer입니다.

변경은 [wiki_vector_repo.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/store/wiki_vector_repo.rs)만 수정했습니다. `list_wiki_vectors()`에서 `query_map` 결과를 더 이상 `.ok()`로 무음 폐기하지 않고, 역직렬화 실패 시 `tracing::warn!`를 남긴 뒤 해당 row만 건너뛰도록 바꿨습니다.

Verification results for Task 02:
✅ `cargo check -p secall-core` — exit 0  
✅ `cargo test -p secall-core --test wiki_indexer` — 4 passed  
✅ `cargo test -p secall-core --lib store::` — 66 passed  
✅ `cargo test -p secall-core --lib wiki::` — 20 passed

## Subtask Results

### 1. Developer Codex · Implementer입니다. Rework는 Task 02 범위만 수정합니다. 지적된 `wiki_vector_repo.rs`의 무음 에러 폐기 경로를 먼저 확인하고, Task 02 검증 명령만 다시 실행하겠습니다.수정 지점은 확인했습니다. `list_wiki_vectors()`의 `query_map` 결과를 순회하면서 역직렬화 에러를 경고로 남기고 해당 row만 건너뛰도록 바꾸겠습니다.패치는 끝났습니다. 이제 Task 02 문서의 검증 명령만 다시 돌려서 rework 결과를 확인하겠습니다.Developer Codex · Implementer입니다.

변경은 [wiki_vector_repo.rs](/Users/d9ng/privateProject/seCall/crates/secall-core/src/store/wiki_vector_repo.rs)만 수정했습니다. `list_wiki_vectors()`에서 `query_map` 결과를 더 이상 `.ok()`로 무음 폐기하지 않고, 역직렬화 실패 시 `tracing::warn!`를 남긴 뒤 해당 row만 건너뛰도록 바꿨습니다.

Verification results for Task 02:
✅ `cargo check -p secall-core` — exit 0  
✅ `cargo test -p secall-core --test wiki_indexer` — 4 passed  
✅ `cargo test -p secall-core --lib store::` — 66 passed  
✅ `cargo test -p secall-core --lib wiki::` — 20 passed

