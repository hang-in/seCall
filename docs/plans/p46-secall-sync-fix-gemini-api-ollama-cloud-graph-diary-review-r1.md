# Review Report: P46 — secall sync 종료 fix + Gemini API 백엔드 제거 + Ollama Cloud 전환 (graph/diary 분리) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-12 04:39
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/vault/config.rs:379 — `SECALL_GRAPH_MODEL` override가 `semantic_backend="ollama_cloud"`일 때도 `graph.ollama_model`에만 저장됩니다. 하지만 실제 cloud 경로는 `graph.cloud_model`만 읽으므로, 환경변수로 cloud 모델을 바꿔도 런타임에 전혀 반영되지 않습니다.
2. crates/secall-core/src/mcp/server.rs:358 — `graph` 패치와 달리 `log` 패치는 `cloud_api_key`를 제거하지 않고 그대로 merge합니다. `/api/config`는 `log.cloud_api_key`를 응답 모델에 포함하고([web/src/lib/api.ts](/Users/d9ng/privateProject/seCall/web/src/lib/api.ts:37)), Settings 저장은 섹션 전체를 다시 PATCH하므로([web/src/routes/SettingsRoute.tsx](/Users/d9ng/privateProject/seCall/web/src/routes/SettingsRoute.tsx:173)), 사용자가 Log 설정만 저장해도 마스킹된 `"<masked>"` 값이나 임의 입력이 실제 config에 덮어써질 수 있습니다.
3. web/src/routes/SettingsRoute.tsx:446 — Log backend 선택지에 `"ollama_cloud"`가 빠져 있어 web UI에서 새 diary backend를 선택할 수 없습니다. Task 05 계약은 Log 섹션에 Ollama Cloud 설정을 노출하는 것이었는데, 현재 구현으로는 CLI/수동 config 편집 없이 기능을 사용할 수 없습니다.

## Recommendations

1. `sync_termination` 테스트는 현재 child drop 동작만 검증합니다. Task 01 계획서대로 `secall sync` 프로세스 종료 자체를 검증하는 회귀 테스트를 별도로 추가하는 편이 안전합니다.
2. Log 섹션에도 `cloud_host`/`cloud_model` 입력과 관련 테스트를 추가해, Graph/Log 모델 분리 요구가 UI에서도 실제로 충족되는지 확인하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | secall sync 미종료 진단 + fix | ✅ done |
| 2 | Gemini API 백엔드 호출 측 제거 | ✅ done |
| 3 | Ollama Cloud 백엔드 도입 | ✅ done |
| 4 | 용도별 기본 모델 매핑 + diary 컨텍스트 가드 | ✅ done |
| 5 | 회귀 테스트 + 문서 + web UI | ✅ done |

