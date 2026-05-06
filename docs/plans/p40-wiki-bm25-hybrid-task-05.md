---
type: task
plan_slug: p40-wiki-bm25-hybrid
task_id: 05
title: 외부 컨트리뷰터 회신 게시 (manual)
parallel_group: D
depends_on: [03, 04]
status: pending
updated_at: 2026-05-06
---

# Task 05 — 외부 컨트리뷰터 회신 게시 (manual)

## Changed files

수정:

- `docs/community/p39-wiki-vector-response.md` — P39 작성 초안. 본 task 에서 다음 정보로 갱신:
  - P40 진행 결정 명시
  - 단순 스코프 (페이지 단위 임베딩, hybrid mode 옵션, BM25 fallback 보존)
  - 머지 PR 번호/링크 (#XX, P40 머지 후)
  - 사용 방법 예 (`?mode=hybrid` 쿼리 + `secall wiki vectorize` 1회 backfill)
  - bge-m3 (Ollama) 의존성 명시

## Change description

### 1. 답변 초안 갱신 항목

P39 task 04 에서 작성한 초안을 다음 구조로 final 화:

```markdown
# Re: Wiki Search BM25 → Hybrid 요청

## 결정: 진행 (P40)

- 스코프: 페이지 단위 임베딩 + hybrid mode (`?mode={keyword|semantic|hybrid}`)
- 기존 keyword 모드 default 유지 (호환)
- 임베딩: bge-m3 via Ollama (recall 과 동일 stack)

## 사용 방법

1. Ollama + bge-m3 준비:
   ```
   ollama pull bge-m3
   ```
2. 한 번 backfill:
   ```
   secall wiki vectorize
   ```
3. hybrid 검색:
   ```
   curl -X POST localhost:3000/api/wiki \
     -d '{"query":"...", "mode":"hybrid"}'
   ```

## 머지 PR

- #XX — feat: P40 wiki 벡터화 (keyword → hybrid)

## 한계 및 향후

- 페이지 단위 (chunker X) — 페이지 100+ 또는 평균 8000+ tokens 도래 후 별도 phase 고려
- BM25 자체는 미도입 — 현재 substring 매칭 유지 (더 큰 변경, 본 phase 외)
- semantic 실패 시 keyword fallback (Ollama 미실행 등 안전망)
```

### 2. 게시 절차 (manual)

본 task 는 코드 변경 없는 **사용자 수동 작업**:

1. P40 plan 의 task 01–04 머지 완료 + PR 번호 확보
2. `docs/community/p39-wiki-vector-response.md` 위 형태로 갱신 (Architect 가 코드처럼 PR 에 포함하거나 별도 commit)
3. 사용자가 GitHub 이슈/PR 코멘트로 답변 게시 (Architect/Developer 가 직접 게시 X)

## Dependencies

- **Task 03 필수** — hybrid mode 동작
- **Task 04 필수** — backfill 명령 동작
- 머지 PR 번호 확보 — 사용자가 PR 머지 후 번호 task 05 본문에 기입

## Verification

이 task 는 코드 변경이 아니므로 자동 검증 명령이 없다. 다음을 매뉴얼로 확인:

```bash
# 1. 답변 초안 파일 갱신 확인
cat docs/community/p39-wiki-vector-response.md | head -30

# 2. P40 PR 번호가 답변에 정확히 기재됐는지 (사용자 게시 전)
grep "#" docs/community/p39-wiki-vector-response.md
```

수동 단계:

- [ ] P40 PR 머지 → 번호 확정
- [ ] `p39-wiki-vector-response.md` 본문에 PR 번호 + 사용 예 final 화
- [ ] 사용자가 GitHub 코멘트 게시 (Architect/Developer 작업 외)

## Risks

- **PR 번호 미확정**: task 03/04 머지 전 게시 X. depends_on 으로 강제.
- **사용 예 부정확**: backfill 명령 실제 출력 (task 04 매뉴얼 검증 결과) 을 답변에 그대로 옮기기.
- **Ollama 의존성 명시 누락**: 컨트리뷰터가 환경 미준비 시 fail → 답변 본문에 명시 필수.

## Scope boundary (수정 금지)

- 모든 코드 파일 (이 task 는 docs only)
- `docs/baseline/p39-p40-decision.md` — P39 의 baseline (이미 final, 변경 X)
- 다른 task 의 산출물
