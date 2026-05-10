---
type: plan
slug: p43-wiki-review-backend-wiki
title: P43 — Wiki review backend 확장 + 로컬 전용 wiki 파이프라인 완성
status: in_progress
version: 1
updated_at: 2026-05-09
canonical: true
---

# P43 — Wiki review backend 확장 + 로컬 전용 wiki 파이프라인 완성

## Description

`crates/secall-core/src/wiki/review.rs:51` 가 `https://api.anthropic.com/v1/messages`
직접 POST + `crates/secall-core/src/wiki/review.rs:36-38` 의 `model_id` 매칭이
`"opus" / "sonnet"` 두 케이스만. `wiki update --review` 가 ANTHROPIC_API_KEY
없는 환경에서 fail. P41 / P42 가 daily diary, graph, embedding 의 5 backend
지원을 끝낸 직후라, review 가 유일한 anthropic-only 잔존 — 로컬 전용 사용자
onboarding 의 마지막 장벽.

본 plan 은 review 도 backend trait (`WikiReviewer`) 으로 추상화하여 5 backend
(claude / codex / haiku / ollama / lmstudio) 모두 지원하고, 부산물로 P42 의
review recommendations + toml_edit (config save 시 주석 보존) + docs 보강을
함께 정리한다.

## Expected Outcome

- `wiki update --review` 가 ollama / lmstudio / claude / codex / haiku 에서 동작
- `[wiki].review_backend` config + `--review-backend` CLI flag (default = `[wiki].default_backend`)
- review prompt 가 backend 별 JSON 강제 instruction 을 자동 inject
- toml_edit 도입 → `Config::save()` 가 사용자 주석/공백 보존 (P41 task 03 / P42 non-goal 회수)
- P42 review recommendations 3건 정리 (가시성 축소, test_backend 분할, rest_config 정규식)
- README + `docs/reference/llm-config.md` 의 review backend matrix 반영

## Subtask Summary

| # | Title | Parallel Group | Depends On |
|---|---|---|---|
| 01 | WikiReviewer trait 도입 | A | — |
| 02 | 5 backend reviewer 구현 | B | 01 |
| 03 | config + CLI 통합 | C | 01, 02 |
| 04 | review prompt JSON 강제 + backend suffix | B | 01 |
| 05 | toml_edit 도입 (config 주석 보존) | A | — |
| 06 | P42 review recommendations 적용 | A | — |
| 07 | Documentation | D | 01–06 |

Task 01, 05, 06 은 서로 disjoint — parallel_group A 로 동시 실행 가능.
Task 02, 04 는 Task 01 의 trait 가 정의된 이후 동시 진행 (parallel_group B).
Task 03 는 Task 02 backend 들이 있어야 통합 가능 (parallel_group C).
Task 07 는 1–6 완료 후 docs 만 갱신 (parallel_group D).

## Constraints

- `ReviewResult` JSON schema 변경 X — downstream consumer (`run_review` in `crates/secall/src/commands/wiki.rs:350,390,481,518`) 영향 0
- default 가 anthropic 유지 → 기존 ANTHROPIC_API_KEY 사용자 회귀 0
- `toml_edit` 은 `Config::save` 만 영향 — `load_or_default` 는 `toml` crate 그대로
- ollama / lmstudio 의 JSON 출력 신뢰성은 prompt engineering 의존 — 파싱 실패 시 1회 retry + friendly fallback
- subprocess (claude / codex CLI) timeout 60초 — wiki backend 와 동일 정책

## Non-goals

- 새 LLM backend 추가 (5종 그대로)
- review prompt content 자체 재작성 — backend suffix 만 추가, 본문 변경 X
- Web UI 의 review backend 선택 — 본 plan 은 CLI/config 한정 (별도 plan)
- `extract_with_anthropic` / `extract_with_ollama` mock 통합 (P42 task 02 미완 — 별도 plan)
- 외부 기여 PR (#26 Codex wiki) — 별도 트랙
- Web SettingsRoute 에 review_backend 노출 — config UI 자체는 P41 task 04 결과 그대로

## References

- P42 review verdict (2026-05-09): `docs/plans/p42-p41-llm-result.md`
- 기존 review hot path: `crates/secall-core/src/wiki/review.rs:19-105`
- 기존 reviewer 호출자: `crates/secall/src/commands/wiki.rs:350,390,481,518`
- 기존 backend impl 5종: `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs`
