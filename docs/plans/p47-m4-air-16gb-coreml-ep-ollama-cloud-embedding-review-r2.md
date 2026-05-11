# Review Report: P47 — M4 Air 16GB 임베딩 부담 완화 (CoreML EP + Ollama Cloud embedding) — Round 2

> Verdict: pass
> Reviewer: 
> Date: 2026-05-12 05:25
> Plan Revision: 0

---

## Verdict

**pass**

## Recommendations

1. `OllamaEmbedder::is_available()`에 대한 HTTP status 분기 회귀를 별도 러스트 테스트로 고정하면, 추후 `reqwest` 호출 경로 변경 시 Task 03 회귀를 더 빨리 잡을 수 있습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ORT CoreML EP 옵션 추가 | ✅ done |
| 2 | OrtEmbedder pool_size 조정 + config 노출 | ✅ done |
| 3 | OllamaEmbedder cloud 모드 지원 | ✅ done |
| 4 | ingest 종료 후 Ollama embed unload | ✅ done |
| 5 | 문서 + web UI + 회귀 테스트 | ✅ done |

