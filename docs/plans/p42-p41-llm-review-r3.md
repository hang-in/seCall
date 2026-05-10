# Review Report: P42 — P41 후속 정리 + LLM 신뢰성 강화 — Round 3

> Verdict: fail
> Reviewer: 
> Date: 2026-05-09 10:05
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall/src/commands/config.rs:405 — `test_lmstudio_backend()` 는 LM Studio URL을 `lmstudio_url()` 에서만 찾는데, 그 helper 는 `wiki.backends.lmstudio.api_url` 과 `log.api_url` 만 확인합니다 (`crates/secall/src/commands/config.rs:489`). 실제 semantic LM Studio 런타임은 `config.graph.ollama_url` 을 사용합니다 (`crates/secall-core/src/graph/semantic.rs:445`). 그래서 graph 쪽에만 LM Studio URL이 설정된 정상 구성에서도 `secall config llm test lmstudio` 가 `SKIP api_url not configured` 를 반환할 수 있어, health check 결과가 실제 동작과 어긋납니다.

## Recommendations

1. LM Studio 검증은 실제 런타임과 같은 설정 소스를 우선 사용하도록 맞추세요. 최소한 `config.graph.ollama_url` 경로를 먼저 보고, wiki/log fallback 은 의도된 경우에만 추가로 허용하는 편이 안전합니다.
2. Task 03 회귀 테스트에 `[graph].ollama_url` 만 설정된 상태에서 `secall config llm test lmstudio --no-network` 가 `OK` 로 나오는 케이스를 추가하세요.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | log.rs 하드코딩 default 정리 | ✅ done |
| 2 | semantic.rs LLM 분기 단위 테스트 | ✅ done |
| 3 | `secall config llm test` 구현 | ✅ done |
| 4 | Settings UI 폴리싱 | ✅ done |
| 5 | REST `/api/config` 추가 회귀 | ✅ done |

