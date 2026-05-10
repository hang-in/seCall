---
type: plan
slug: p42-p41-llm
title: P42 — P41 후속 정리 + LLM 신뢰성 강화
status: in_progress
version: 1
updated_at: 2026-05-09
canonical: true
---

# P42 — P41 후속 정리 + LLM 신뢰성 강화

## Description

P41 ("LLM 설정 통합 + Daily diary 다중 백엔드 + Web 설정 화면") 리뷰는 통과했으나
다음 잔존 항목이 남았다:

1. `crates/secall/src/commands/log.rs` 의 fallback 3건 (line 236, 246, 280) 이
   `defaults.rs` 의 상수 (`WIKI_CLAUDE_DEFAULT`, `WIKI_CODEX_DEFAULT`,
   `GRAPH_LMSTUDIO_DEFAULT`) 가 정의됐음에도 여전히 리터럴 사용 — drift 위험.
2. code-review-graph 가 보고한 high-risk 미테스트 함수 5건:
   `extract_with_gemini`, `extract_with_llm`, `warn_using_default`,
   `warned_fields`, `rest_router`. 그 중 `extract_with_*` 가 graph semantic
   추출 hot path.
3. Task 05 의 stretch goal `secall config llm test` (백엔드 ping) 미구현 —
   사용자가 backend 인증/연결을 사전 검증할 방법 없음.
4. P41 Settings UI 의 dirty state, masked input UX, validation 보강 여지.
5. `/api/config` REST 핸들러의 edge case (graph 의 `gemini_api_key` 무시,
   섹션 보존, 잘못된 JSON 400) 회귀 부족.

본 plan 은 위 5건을 task 1개씩 분리해 빠르게 정리하고, 그 위에 쌓일
다음 기능 (P43 wiki Pro 3 업그레이드 등) 의 신뢰 기반을 만든다.

## Expected Outcome

- `log.rs` 의 모든 backend fallback 이 `defaults.rs` 상수 사용 (drift 차단).
- `extract_with_gemini` / `extract_with_llm` 의 backend 분기 + 모델 fallback 단위 테스트 추가 (mockito 기반).
- `secall config llm test [<backend>]` 로 인증/연결 사전 검증 가능 (CI skip 옵션 포함).
- Settings UI 의 save 후 dirty state 자동 리셋 + masked input 안내 + inline validation.
- REST `/api/config` 의 edge case 3종 회귀 추가 (총 7 케이스).

## Subtask Summary

| # | Title | Parallel Group | Depends On |
|---|---|---|---|
| 01 | log.rs 하드코딩 default 정리 | A | — |
| 02 | semantic.rs LLM 분기 단위 테스트 | A | — |
| 03 | `secall config llm test` 구현 | A | — |
| 04 | Settings UI 폴리싱 | B | — |
| 05 | REST `/api/config` 추가 회귀 | A | — |

5 task 모두 변경 파일이 disjoint 하므로 parallel 실행 가능
(task 04 만 web/, 그 외는 crates/).

## Constraints

- `log.rs::run(date, backend, model)` 시그니처 변경 X — task 01 의 검증된 인터페이스 유지.
- `secall config llm test` 는 외부 네트워크 의존 — `--no-network` 또는 mock-only 옵션 제공.
- toml_edit 도입 (주석 보존) 은 본 plan 영역 외 — 별도 plan.
- Settings UI 카테고리 추가 / 라우트 분할 X — 기존 4 섹션 (Wiki/Graph/Log/Embedding) 유지.

## Non-goals

- 새 LLM backend 추가 (gemini-cli, openrouter 등)
- wiki Pro 3 / 3.1 모델 업그레이드 — 별도 plan
- toml 주석 보존 (`toml_edit` 도입) — 별도 plan
- REST `/api/config` 의 secret PATCH 경로 (의도적으로 차단 유지)
- `extract_with_anthropic` / `extract_with_ollama` 의 별도 단위 테스트 (task 02 의 분기 테스트로 통합)

## References

- P41 리뷰 verdict (2026-05-09): `docs/plans/p41-llm-daily-diary-web-result.md`
- code-review-graph (P41 종료 시점): risk 0.60, 58 test gap, 5 untested hot-path
- 본 plan 은 P41 의 새 기능 채택 직후 1주일 안에 완료 권장 (drift 최소화)
