# Review Report: P47 — M4 Air 16GB 임베딩 부담 완화 (CoreML EP + Ollama Cloud embedding) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-12 05:15
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/search/embedding.rs:98 — `OllamaEmbedder::is_available()` 가 `request.send().await.is_ok()` 만 확인해서 HTTP 401/403/404도 "available"로 처리합니다. 그 결과 `crates/secall-core/src/search/vector.rs:495` 에서 `ollama_cloud` 임베더를 준비 완료로 로그한 뒤, 실제 첫 `/api/embed` 호출에서만 실패하게 됩니다. Task 03 의도대로 "Cloud 미가용 시 조기 fallback/경고"가 되지 않는 구체적 런타임 결함입니다.
2. web/src/routes/SettingsRoute.tsx:108 — Graph 섹션은 `Cloud host` 입력 필드에 URL 에러를 표시하지만, `sectionErrors.graph` 계산에서 `graphForm.cloud_host` 검증을 누락했습니다. 그래서 같은 파일의 `Cloud host` 필드(`web/src/routes/SettingsRoute.tsx:383`)에 에러가 보여도 저장 버튼은 비활성화되지 않아, 잘못된 Cloud host 값을 그대로 저장할 수 있습니다.

## Recommendations

1. `OllamaEmbedder::is_available()` 는 최소한 `resp.status().is_success()` 를 확인하고, 가능하면 401/403/404를 구분해 fallback 로그를 더 명확히 남기는 편이 안전합니다.
2. Task 05 계약과 맞추려면 `web/src/routes/SettingsRoute.test.tsx` 에 embedding 섹션의 `ollama_cloud` 선택지, `Embedding cloud host/model` 검증, `Pool size` 입력 동작을 직접 검증하는 테스트를 추가하는 편이 좋습니다.
3. Task 02 계약상 `pool_size` 관련 설정 round-trip / 휴리스틱 테스트가 의도보다 약합니다. `resolve_pool_size()` 와 `embedding.pool_size` 저장/로딩 케이스를 별도 테스트로 고정하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | ORT CoreML EP 옵션 추가 | ✅ done |
| 2 | OrtEmbedder pool_size 조정 + config 노출 | ✅ done |
| 3 | OllamaEmbedder cloud 모드 지원 | ✅ done |
| 4 | ingest 종료 후 Ollama embed unload | ✅ done |
| 5 | 문서 + web UI + 회귀 테스트 | ✅ done |

