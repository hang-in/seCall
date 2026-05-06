---
type: plan
status: draft
updated_at: 2026-05-05
slug: p39-wiki-sync-auto-commit-fix
version: 1
---

# P39 — Wiki 파이프라인 대규모 실행 검증 + sync auto-commit fix

## Description

683 세션 sync (실행 중) 결과를 baseline 으로 wiki 파이프라인 안정성/비용/품질 측정. 동시에 sync 진행 중 발견된 secall 의 vault auto-commit 결함 (3회 누적 "auto: uncommitted vault changes" + 매번 일부 누락) hot-fix. 외부 컨트리뷰터 요청 (BM25 → vector hybrid) 의 P40 진행 여부 데이터도 본 phase 가 수집.

## 현재 한계

- `crates/secall-core/src/vault/git.rs:146` — `auto_commit` 가 `git add raw/ wiki/ index.md log.md .gitignore` 만 stage. **누락 확인됨**: `SCHEMA.md`, `graph/`, `log/` (디렉터리). 매 sync 마다 일부 누락 → `git pull --rebase` 실패 누적.
- wiki 파이프라인 baseline 부재 — 처리 페이지 수, 비용/시간, edge case 빈도 측정 데이터 없음.
- 외부 컨트리뷰터 요청 (wiki 벡터화) 우선순위 결정에 필요한 데이터 (페이지 수/길이/검색 빈도) 없음.

## Expected Outcome

- `auto_commit` 가 vault 의 모든 modified + untracked 정확히 잡음 → `git pull --rebase` 사고 재발 X
- `docs/baseline/p39-wiki-baseline.md` — 처리 페이지 수 / 비용·시간 / edge case 빈도
- `docs/baseline/p39-wiki-quality.md` — 무작위 10 페이지 spot check 결과
- `docs/baseline/p39-p40-decision.md` — P40 (wiki 벡터화) 우선순위 결정 데이터 + 결론
- 외부 컨트리뷰터 답변 초안 (`docs/community/p39-wiki-vector-response.md`)
- Insight finding `STA-vault-auto-commit-누락` 신규 등록 + Task 01 fix 후 status `resolved`

## Subtasks

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 00 | sync auto-commit 로직 fix (hot-fix) | A | — |
| 01 | wiki 파이프라인 baseline 측정 보고서 | B | sync 완료 |
| 02 | wiki 페이지 품질 spot check | B | sync 완료 |
| 03 | wiki 콘텐츠 양 측정 + P40 우선순위 데이터 | B | sync 완료 |
| 04 | README + Insight findings + 컨트리뷰터 답변 초안 | C | 00, 01, 02, 03 |

병렬 실행 전략:
- Phase A — Task 00 (sync 무관, 즉시 시작 가능)
- Phase B — Task 01 + 02 + 03 (sync 완료 후 동시 dispatch — 다른 보고서 파일)
- Phase C — Task 04 (모든 task 완료 후)

## Constraints

- **수정 금지**: P32~P38 production 코드 동작 변경. Task 01 의 `vault/git.rs::auto_commit` 만 production 수정.
- sync 진행 중 vault dir 의 `git add/commit` 동시 실행 금지 (race). Task 01 은 코드 작성 + 단위 테스트 까지만, 사용자 vault 에 적용 (재 sync) 은 사용자 결정.
- 측정/보고서 task (02/03/04/05) 는 sync 완료 후 진행.
- 외부 컨트리뷰터 답변은 게시 X — 사용자 검토 후 직접 게시.

## Non-goals

- wiki 벡터화 자체 (P40 별도 phase 결정)
- LLM backend 변경 / 프롬프트 튜닝
- vault git workflow 전면 재설계 (auto-commit 단일 hot-fix 만)
- coverage 측정 도구 도입
- 자동 wiki 스케줄러 / cron

## Success criteria

- `cargo test --test vault_auto_commit` — 신규 회귀 테스트 통과 (M/??/D 다양한 상태 commit 후 status clean 검증)
- baseline 보고서 3건 (`p39-wiki-baseline.md`, `p39-wiki-quality.md`, `p39-p40-decision.md`) 모두 측정 항목 채워짐
- README v0.8.2 changelog 행 + Insight finding status 갱신
- 외부 컨트리뷰터 답변 초안 사용자 검토 가능
