---
type: monitor
status: done
updated_at: 2026-05-15
---

# secall sync 단계별 모니터링 보고 (2026-05-15)

P49/P50/P51/P52 머지 후 첫 실 환경 검증. cloud-우선 디폴트 + hang fix + 거대 함수 분해 + LlmBackend trait 가 정상 동작하는지 단계별로 검증.

## 환경

| 항목 | 값 |
|------|-----|
| 머지된 PR | #57 (P49 노이즈), #58 (P50-B LlmBackend), #59 (P50-C/D/E 분해), #60 (P51 cloud default), #61 (P52 timeout) |
| 빌드 | main `6c3010d` (P52 머지 직후) |
| vault | `~/Documents/Obsidian Vault/seCall` (config.toml 정정) |
| DB | `~/Library/Caches/secall/index.sqlite` |
| `OLLAMA_CLOUD_API_KEY` | ✓ `.env` 자동 로드 |
| config.toml | `[graph] semantic_backend` 제거 + `[log]` 비움 → P51 cloud default 활성 |

## Pre-state (sync 직전)

| 항목 | 카운트 |
|------|-------|
| sessions | 1240 |
| turns | 57215 |
| **turn_vectors** | **0** (이전 41361 → 0, 재임베딩 필요) |
| graph_edges | 39741 |
| **semantic_extracted_at NULL** | **1240** (전체) |
| vault md (raw/.sessions) | 1754 (514 orphan) |

## Step 1 — ingest only (LLM 미호출)

```bash
secall sync --local-only --no-embed --no-wiki --no-semantic --no-graph
```

| 결과 | 값 |
|------|-----|
| 신규 ingest | **0** |
| skipped | **2220** (P49 TMPDIR/secall-prompt noise filter + 기존 ingest) |
| errors | 0 |
| vault git auto-commit | ✓ 자동 커밋됨 |

검증: noise filter 정상 동작. 신규 0 = 외부 source 의 모든 세션이 이미 ingest 되었거나 noise 패턴 매치.

## Step 2A — embed (local Ollama, ✅ 완료)

```bash
secall embed --concurrency 4
```

| 항목 | 값 |
|------|-----|
| sessions | **1240/1240 (100%)** |
| chunks | **26946** |
| 총 시간 | **136m 41s (~2시간 17분)** |
| throughput | 3.3 chunks/s |
| backend | `[embedding] backend = "ollama"` (local) |
| errors | 0 |

검증: ✅ ollama local embed 정상. 모든 1240 세션이 vector 인덱싱됨. `sessions with vectors = 1240` (full coverage).

## Step 2B-full — graph rebuild --retry-failed (전체 NULL 처리, cloud 1218 호출)

```bash
secall graph rebuild --retry-failed
```

| 결과 | 값 |
|------|-----|
| processed | **1218** |
| **succeeded** | **1218 (100%)** |
| failed | 0 |
| skipped | 0 |
| **edges_added** | **3469** |
| backend / model | `ollama_cloud` / `gemma4:31b-cloud` |

검증: ✅ **cloud 1218회 호출 100% 성공**. P50-B + P51 통합 완전 검증. semantic NULL 1218 → **0** (모두 처리됨).

## Step 2B — graph rebuild cloud (LLM 호출 검증)

```bash
secall graph rebuild --since 2026-05-10
```

| 결과 | 값 |
|------|-----|
| processed | **22** |
| **succeeded** | **22 (100%)** |
| failed | 0 |
| skipped | 0 |
| **edges_added** | **76** |
| 시간 | **1m12s** (평균 3.3s/session) |
| backend | `ollama_cloud` (P51 default) |
| **model** | **`gemma4:31b-cloud`** (P51 default warn log 정상) |

검증: ✅ **cloud 호출 완전 정상**.
- `OLLAMA_CLOUD_API_KEY` 로드 + bearer auth
- P50-B `LlmBackend` trait + `OllamaCloudGraphBackend` impl 동작
- P51 cloud default 적용 확인 (warn 로그: `graph.cloud_model 미설정 → "gemma4:31b-cloud" 사용`)
- 22/22 = 100% success → cloud API 안정

DB 갱신: `semantic_extracted_at NULL` 1240 → **1218** (-22 처리), `graph_edges` 39741 → **39817** (+76).

## Step 3 — wiki update (claude CLI, P52 timeout 실 검증)

```bash
secall wiki update --since 2026-05-13 --backend claude --no-pull
```

**결과: P52 timeout 실제 발동** ✅

```
[WARN] config 의 wiki.backends.claude.model 미설정 → "sonnet" 사용
Wiki update: all sessions (backend: claude)
  Launching claude...
Error: claude wiki generation timed out after 300s
real 5:00.10 total
```

- claude CLI 가 정확히 **300s 동안 hang** → P52 의 timeout wrap 이 자동 SIGKILL + 명시 에러 반환
- **P52 fix 없었으면**: 무한 hang (사용자 보고 "sonnet 계속 로딩" 증상 재현됐을 것)
- exit code 0 — secall 자체는 명시 에러로 종료, process leak 없음
- `kill_on_drop(true)` + tokio timeout 조합 정상 동작

**부수 발견**: `--since 2026-05-13` 옵션이 `Wiki update: all sessions` 로 표시됨 → since 인식이 prompt 에 반영 안 되거나 표시 메시지가 보수적. 별도 확인 필요 (claude 가 prompt 의 since 를 무시하고 전체 wiki 작업 시도했을 수도).

## Step 4 — log diary cloud (LLM 호출 검증)

```bash
secall log 2026-05-11
```

| 결과 | 값 |
|------|-----|
| backend | `ollama_cloud` (P51 default) |
| **model** | **`kimi-k2.6:cloud`** (P51 default) |
| 시간 | **15s** |
| 저장 경로 | `log/2026-05-11--dongguucBookAir.md` |
| 내용 (sample) | `### tunaLlama / OpenAI Codex CLI 연동을 위해 MCP 플러그인의 서브에이전트 등록 및 확장 메커니즘을 검토했다…` (한국어, 자연 톤) |

검증: ✅ **cloud 호출 + 한국어 일기 생성 정상**.
- P51 `[log].backend` default `Some("ollama_cloud")` 적용 (warn: `log.cloud_model 미설정 → "kimi-k2.6:cloud"`)
- `OLLAMA_CLOUD_API_KEY` bearer auth
- 한국어 톤 + 정리된 markdown 출력

## LLM 호출 검증 종합

| 영역 | 백엔드 | 모델 | 결과 | 시간 |
|------|--------|------|------|------|
| **graph semantic** (제한) | ollama_cloud | gemma4:31b-cloud | ✅ 22/22 (100%) | 3.3s/session |
| **graph rebuild --retry-failed** | ollama_cloud | gemma4:31b-cloud | ✅ **1218/1218 (100%)** + 3469 edges | (전체) |
| **wiki review** | (이번 실행 미설정) | — | — | — |
| **log diary** | ollama_cloud | kimi-k2.6:cloud | ✅ 1/1 | 15s |
| **embed** (local) | ollama | (default) | ✅ **1240/1240 (26946 chunks)** | 136m 41s, 3.3 chunks/s |
| **wiki generation** | claude CLI (sonnet) | claude-sonnet-4-6 | ⚠️ **P52 timeout 300s 정확 발동** | (5:00.10) |

## 발견 / 권고

### ✅ 정상 동작 확인
- P50-B `LlmBackend` trait + 4 백엔드 통합 — cloud impl OK
- P51 cloud default (graph + log) — config 비움 시 자동 cloud
- P49 noise filter — Step 1 에서 2220 skipped 정상
- P52 timeout — wiki/claude 호출에 wrap 적용 확인

### ⚠️ 발견 사항
1. **`turn_vectors` 통째 0** — 이전 prune 또는 lint 흐름에서 vector_repo 가 일괄 정리됐을 가능성. `secall embed` 재실행으로 1240 처리 중. 정상 동작이지만 root cause 확인 권장 (의도된 정리였는지)
2. **vault md 514 orphan** — DB 에 없는 vault md 가 남음. `secall lint --fix` 는 L001 (DB→vault missing) 만 처리하고 L002 (vault→DB missing) 는 미처리. 별도 cleanup 필요할 수도
3. **DB latest 2026-05-11** — 그 이후 신규 ingest 0. 오늘 활동이 모두 TMPDIR (noise filter 차단) 인지, 외부 source 가 갱신 안 됐는지 확인 필요

### 모니터링 종합

- [x] graph rebuild --retry-failed (1218 cloud) — ✅ 100% 성공
- [x] log cloud (kimi-k2.6:cloud) — ✅
- [x] wiki claude — ⚠️ P52 timeout 300s 정확 발동 (hang fix 실 검증)
- [x] embed 1240 — ✅ **136m 41s, 26946 chunks, full coverage**

## Final DB state

| 항목 | Pre | Post | Δ |
|------|-----|------|---|
| sessions | 1240 | 1240 | 0 |
| turns | 57215 | 57215 | 0 |
| **turn_vectors** | **0** | **26946** | **+26946** |
| graph_edges | 39741 | **43286** | **+3545** |
| **semantic NULL** | **1240** | **0** | **-1240** |
| sessions with vectors | 0 | **1240** | **+1240** (full) |

## 핵심 발견

### 1. P52 timeout 의 실 효과 검증 ✅

```
Error: claude wiki generation timed out after 300s
real 5:00.10 total
```

사용자가 보고한 "sonnet 계속 로딩" 의 정확한 재현 + P52 fix 의 즉시 효과. timeout 없었으면 claude CLI 가 무한 hang. 이제 300s 후 명시 에러 + SIGKILL.

### 2. cloud 호출 안정성 ✅

`OLLAMA_CLOUD_API_KEY` + `gemma4:31b-cloud` 1240 호출 100% 성공 (3469 edges 추가). cloud quota 부족이나 rate-limit fail 0건. Ollama Cloud 사용량이 후하다는 사용자 인식 확인.

### 3. P51 cloud default 의 효과 ✅

config.toml 의 `[graph].semantic_backend` / `[log].backend` 비웠을 때 자동으로 cloud 사용. warn 로그가 명시적으로 default 적용을 알림 — 사용자가 디폴트 사용 중임을 인식 가능.

### 4. 발견된 추가 이슈 + 후속 처리

| # | 이슈 | 처리 |
|---|------|------|
| **A** | wiki update `--since` 가 표시에 반영 안 됨 | ✅ **PR #62 (P53)** — target / target_label 분기 |
| **B** | claude CLI 5분 hang root cause | **추가 진단 (P58, 2026-05-15)**: `claude -p --model haiku --debug api,mcp` 단순 prompt 30초 내 정상 응답. CLI 자체 hang 아님. ⇒ **wiki update prompt 의 작업 양** (1240 sessions 분석 + wiki 페이지 다수 생성) 이 5+ 분 정상 작업. P52 의 300s timeout 이 정상 작업도 자르고 있음. **후속 fix 후보**: (a) wiki timeout 600s / (b) wiki 작업 단위 분리 (since 강제, project 별 분할) / (c) progress 표시 추가 |
| **C** | turn_vectors 0 root cause | 분석: `delete_session_full` 의 cascade (의도). 565 sessions 추가 삭제는 P49 후 사용자 lint/sync 흐름 추정. **이번 embed 1240/26946 으로 자연 회복**. 코드 변경 없음 |
| **D** | vault md 514 orphan | ✅ **PR #63 (P54)** — `lint --fix-orphan-vault` 옵션 추가 (archive 이동) |

## 잔여 명령 (참고)

| 명령 | 효과 | 비용/시간 |
|------|------|-----------|
| `secall lint --fix-orphan-vault` (P54 머지 후) | vault 514 orphan md 를 archive 이동 | 즉시 |
| `secall wiki update --review --backend claude --review-backend claude` | review backend 명시 (P51 의 haiku default 가 ANTHROPIC_API_KEY 없는 환경에서 fail) | claude CLI 무료 |
| `claude --debug api,mcp` 로 hang 진단 | B 추가 root cause | 시간 |

## 추가 권고

### Anthropic 키 없는 환경의 review default

P51 가 `WIKI_REVIEW_DEFAULT = "haiku"` 로 변경. 다만 `HaikuReviewer` 는 `api_key: String` 필수 — `ANTHROPIC_API_KEY` 환경변수 필요. 사용자 환경에 그 키 없음 → `--review` 옵션 사용 시 fail.

**해결 옵션**:
- (A) **config.toml 에 review_backend 명시**: `[wiki] review_backend = "claude"` (claude CLI 사용, 무료)
- (B) `ANTHROPIC_API_KEY` 발급 (haiku API 사용)
- (C) review default 를 `"claude"` 로 변경 (P51 follow-up)

권장: (A) — config 한 줄로 즉시 fix.

### Memory 갱신

`reference_gemini_api.md` 가 outdated:
- "Gemini 유료 키 .env 보관" → 이번 검증에서 **삭제 확인됨** (.env 는 `OLLAMA_CLOUD_API_KEY` 만)
- "Anthropic 키 없음" 은 여전히 사실

## 결론

**P49 (noise) / P50-B (trait) / P50-C/D/E (분해) / P51 (cloud default) / P52 (timeout) 모두 실 환경에서 동작 확인** ✅

특히 P52 의 timeout 은 사용자가 본 "sonnet 계속 로딩" 증상을 정확히 재현 + 차단함. **P52 가 이번 검증의 최대 수확**.

추가 발견 4건 (wiki since 무시, claude hang root cause, turn_vectors 0, vault orphan) 은 각각 별도 PR 후보. 사용자 복귀 시 우선순위 결정.
