# Review Report: P39 — Wiki 파이프라인 대규모 실행 검증 + sync auto-commit fix — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-06 10:26
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/vault/git.rs:168 — Task 00은 `auto_commit()`만 `git add -A`로 고쳤지만, sync 마지막 `push()` 경로는 여전히 `raw/`, `wiki/`, `index.md`, `log.md`만 stage합니다. 그래서 sync 도중 생성·수정된 `graph/`, `log/`, `SCHEMA.md` 같은 경로는 이번 실행의 push에 포함되지 않고 다음 sync 시작 시점까지 dirty 상태로 남을 수 있어, Task 00이 문서에서 약속한 "누락 경로를 모두 포착" 목표를 완전히 충족하지 못합니다.

## Recommendations

1. docs/baseline/p39-p40-decision.md:113 — 결정이 `P40 즉시 진행`인데 후속 액션에는 아직 `보류 결정` 분기가 남아 있습니다. 문서 소비자가 헷갈리지 않도록 선택되지 않은 분기는 제거하거나 "비적용"으로 명확히 표시하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | sync auto-commit 로직 fix (hot-fix) | ✅ done |
| 2 | wiki 파이프라인 baseline 측정 보고서 | ✅ done |
| 3 | wiki 페이지 품질 spot check | ✅ done |
| 4 | wiki 콘텐츠 양 측정 + P40 (wiki 벡터화) 우선순위 데이터 | ✅ done |
| 5 | README + Insight findings + 컨트리뷰터 답변 초안 | ✅ done |

