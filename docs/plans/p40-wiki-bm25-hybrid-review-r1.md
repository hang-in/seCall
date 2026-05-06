# Review Report: P40 — Wiki 벡터화 (BM25 → hybrid) — Round 1

> Verdict: conditional
> Reviewer: 
> Date: 2026-05-06 11:35
> Plan Revision: 0

---

## Verdict

**conditional**

## Findings

1. crates/secall-core/src/store/wiki_vector_repo.rs:105 — `list_wiki_vectors()`에서 `.filter_map(|row| row.ok())`로 행 역직렬화 에러를 무음 폐기. BLOB이 손상된 페이지가 있을 경우 semantic/hybrid 결과에서 경고 없이 누락됨. 프로젝트 규칙("Do NOT silently ignore errors") 위반. 최소 fix: `Err(e) => { tracing::warn!("wiki_vectors row error: {e}"); None }` 분기 추가.

## Recommendations

1. collect_semantic_matches 호출마다 env var로 OllamaEmbedder를 새로 생성함. 요청 빈도가 높아질 경우 Server 초기화 시 URL/모델을 캐싱하는 방향 검토.
2. wiki_search_modes.rs의 `test_semantic_fallback_on_embed_failure`가 포트 9(discard)에 연결 시도 — 네트워크 격리 환경에서 타임아웃이 길어질 수 있음. `connect_timeout` 제한을 OllamaEmbedder에 추가하는 것을 향후 고려.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB v9 마이그레이션 | ✅ done |
| 2 | Wiki indexing | ✅ done |
| 3 | Search hybrid mode | ✅ done |
| 4 | Migration 명령 | ✅ done |
| 5 | 외부 컨트리뷰터 회신 게시 (manual) | ✅ done |

