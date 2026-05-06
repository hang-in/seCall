---
type: community-response
status: draft
updated_at: 2026-05-06
plan_slug: p40-wiki-bm25-hybrid
---

# Re: Wiki Search BM25 → Hybrid 요청

## 결정: 진행 (P40)

- 스코프: 페이지 단위 임베딩 + hybrid mode (`?mode={keyword|semantic|hybrid}`)
- 기존 keyword 모드 default 유지
- 임베딩 backend: bge-m3 via Ollama
- semantic / hybrid 실패 시 keyword fallback 유지

## 답변 초안

좋은 지적 감사합니다.

이번 P40에서 wiki_search 쪽도 벡터 기반 검색을 추가하기로 결정했습니다. 다만 범위는 의도적으로 단순하게 잡았습니다. 우선은 **페이지 단위 임베딩**만 넣고, 검색 모드는 `keyword` / `semantic` / `hybrid` 세 가지로 제공합니다. 기본값은 기존 호환을 위해 그대로 `keyword` 입니다.

구현은 recall 쪽과 같은 bge-m3 + Ollama 스택을 재사용합니다. `semantic` 은 페이지 임베딩 기반 유사도 검색이고, `hybrid` 는 기존 keyword 결과와 semantic 결과를 결합합니다. Ollama가 내려가 있거나 임베딩 호출이 실패하면 keyword fallback 으로 내려가도록 안전망도 넣었습니다.

사용 방법은 다음과 같습니다.

1. `ollama pull bge-m3`
2. `secall wiki vectorize`
3. `curl -X POST localhost:3000/api/wiki -H 'content-type: application/json' -d '{"query":"git 자동화","mode":"hybrid"}'`

머지 PR 번호는 현재 구현 완료 후 확정 대기 중이라, 아래 항목만 merge 직후 채워서 게시하면 됩니다.

## 머지 PR

- `#TBD` — feat: P40 wiki 벡터화 (keyword → hybrid)

## 게시 전 체크

- [ ] PR 번호 확정 후 `#TBD` 교체
- [ ] 예시 명령이 현재 릴리스와 일치하는지 최종 확인
- [ ] GitHub 댓글로 게시

## 인용 데이터 출처

- `docs/baseline/p39-p40-decision.md`
- `docs/plans/p40-wiki-bm25-hybrid.md`
