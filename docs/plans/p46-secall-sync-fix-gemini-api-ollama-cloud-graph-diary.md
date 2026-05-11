---
type: plan
status: draft
updated_at: 2026-05-12
plan_id: P46
slug: p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary
---

# P46 — secall sync 종료 fix + Gemini API 백엔드 제거 + Ollama Cloud 전환 (graph/diary 분리)

## 배경

P45 완료 후 사용자가 보고한 3가지 미해결 항목을 한 plan 에 묶어 처리합니다.

1. **(시급)** `secall sync` 가 wiki 생성 완료 후 종료되지 않아 Claude API 사용량이 계속 차감되는 버그.
2. **(요청)** 호출하는 쪽 Gemini API 백엔드 코드 전부 제거. **단, `ingest/gemini.rs` / `ingest/gemini_web.rs` / `detect::find_gemini_sessions` 는 외부 Gemini 세션 ingest 용도이므로 유지.**
3. **(전환)** Ollama Cloud 백엔드 도입 + 용도별 모델 분리:
   - graph (semantic 추출, JSON 응답): `gemma4:31b-cloud`
   - diary (한국어 일기): `kimi-k2.6:cloud` (128k 컨텍스트 한계 → 입력 가드 필요)
   - API key 환경변수: `OLLAMA_CLOUD_API_KEY` (.env 에 입력 완료)

## 기대 결과

- `secall sync` 가 wiki 작업 완료 후 즉시 프로세스 종료 (exit 0, 추가 API 호출 없음).
- `semantic_backend = "gemini"` / `log.backend = "gemini"` 옵션 제거, `SECALL_GEMINI_API_KEY` 참조 제거.
- `semantic_backend = "ollama_cloud"` 동작, graph/log 가 각각 다른 cloud 모델 호출.
- diary 입력 길이가 임계치 (보수치 100k 토큰 ≈ 400k chars) 초과 시 오래된 turn 부터 자르기 + 경고 로그.
- web SettingsRoute 에서 Gemini API key 입력 제거, Ollama Cloud API key 입력 추가 (env: `OLLAMA_CLOUD_API_KEY`).
- 모든 unit/integration 테스트 통과, `docs/reference/llm-config.md` / `README.md` / `README.en.md` 업데이트.

## 서브태스크

| # | 제목 | 의존성 | parallel_group |
|---|------|--------|----------------|
| 01 | secall sync 미종료 진단 + fix | — | 1 |
| 02 | Gemini API 백엔드 호출 측 제거 | — | 1 |
| 03 | Ollama Cloud 백엔드 도입 | 02 | 2 |
| 04 | 용도별 기본 모델 매핑 + diary 컨텍스트 가드 | 03 | 3 |
| 05 | 회귀 테스트 + 문서 + web UI | 04 | 4 |

- Task 01 (sync fix) 는 Task 02 와 영역이 분리되어 있어 **병렬 가능**.
- Task 03~05 는 Ollama Cloud 도입 위에 차곡차곡 쌓이는 순차 진행.

## 제약 사항

- **Anthropic 키 없음** → Anthropic 의존 추가 금지.
- **`ingest/gemini.rs`, `ingest/gemini_web.rs`, `detect::find_gemini_sessions` 는 수정 금지** — 외부 Gemini 세션 ingest 용도라 마이그레이션과 무관.
- Ollama Cloud 모델 태그(`gemma4:31b-cloud`, `kimi-k2.6:cloud`)는 사용자가 알려준 값. **카탈로그 변동 가능성에 대비해 config override 가 반드시 가능해야 함.** 코드 하드코딩 X, defaults 만 둠.
- env var 이름은 `OLLAMA_CLOUD_API_KEY` 로 통일. `SECALL_` prefix 붙이지 말 것.
- `secall config test` 출력에서 gemini 줄 제거, ollama_cloud 줄 추가.

## Non-goals

- secall-web 정합성/UX 묶음 (turn_count stale, 'T' project, tool-only turn 노이즈, SessionListItem compact) — 별도 P47 로 분기.
- wiki 백엔드 변경 — wiki 는 이미 ollama / claude / codex / haiku / lmstudio / anthropic 만 지원 (Gemini 없음). 이 plan 의 범위 아님.
- Gemini 세션 ingest 코드 제거 — 위 제약 참조.
- 정확한 토큰화 도입 — diary 컨텍스트 가드는 char 기반 근사로 충분, tiktoken / HF tokenizer 도입 X.
- Ollama Cloud 모델 품질 벤치마크 — 사용자가 직접 사용해보고 후속 P 에서 조정.

## 작업 지시서

- [Task 01 — secall sync 미종료 진단 + fix](./p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-task-01.md)
- [Task 02 — Gemini API 백엔드 호출 측 제거](./p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-task-02.md)
- [Task 03 — Ollama Cloud 백엔드 도입](./p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-task-03.md)
- [Task 04 — 용도별 기본 모델 매핑 + diary 컨텍스트 가드](./p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-task-04.md)
- [Task 05 — 회귀 테스트 + 문서 + web UI](./p46-secall-sync-fix-gemini-api-ollama-cloud-graph-diary-task-05.md)
