---
type: community-response
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
---

# Wiki 벡터화 답변 (외부 컨트리뷰터, 2026-05-05)

## 원 댓글

> "조금 다른 방향의 질문인데 secall recall은 벡터 방식이고 wiki_search는
> bm25기반 검색인데 wiki 벡터화 계획이 있으신지요?"

## 답변 초안 (한국어, 친절 + 짧음 + 기술 정확)

좋은 지적 감사합니다.

현재 wiki 규모가 19 페이지 / 평균 약 4,870 토큰 (bimodal — 짧은 페이지 다수 + 긴 합본 페이지 소수) 정도라 솔직히 BM25 단독으로도 아직 한계가 뚜렷하지 않은 상태입니다. recall (세션 단위 벡터) 과 wiki_search (BM25) 의 검색 모델이 다른 게 일관성 측면에서는 분명한 약점이라, **다음 phase (P40) 에서 wiki 벡터화를 진행하기로 결정**했습니다.

진행 결정 근거는 세 가지입니다.

1. **일관성** — recall ↔ wiki_search 검색 경험을 동일한 시맨틱 모델로 통일.
2. **early infrastructure** — 페이지 수가 적을 때 임베딩 파이프라인을 미리 깔아두면, 향후 wiki 가 커져도 점진 확장 가능.
3. **외부 신호** — 동일 관심사를 외부에서 짚어주신 만큼 우선순위를 끌어올렸습니다.

스코프는 의도적으로 단순하게 잡았습니다 — **페이지 단위 임베딩** (별도 chunker 분리 없이 페이지 본문 1개 = 임베딩 1개) 으로 시작합니다. wiki 가 커지면 그때 chunk 전략을 추가할 예정입니다.

시점은 P39 (현재 진행 중인 sync auto-commit fix + baseline 측정) 머지 직후 P40 plan 으로 진행합니다. 진행 상황은 GitHub issues / commit 로 공유드리겠습니다.

다시 한 번 좋은 질문 감사드립니다. 추가 의견 있으시면 편하게 댓글 달아주세요.

## 게시 결정 (사용자)

- [ ] 게시 OK — GitHub 댓글에 위 답변 그대로
- [ ] 수정 필요 — 톤/내용 조정 메모:
  - ...

## 게시 후 메모

- 게시 일시: (TBD)
- 후속 응답: (TBD)

## 인용 데이터 출처

- 페이지 수 / 평균 토큰 / bimodal 분포: `docs/baseline/p39-p40-decision.md`
- P40 진행 결정 근거: `docs/baseline/p39-p40-decision.md` (사용자 strategic 결정)
- 단순 스코프 정의 (페이지 단위 임베딩, chunker 분리 X): 동 문서
