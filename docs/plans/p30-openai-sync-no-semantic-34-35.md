---
type: plan
status: in_progress
updated_at: 2026-04-17
version: 1
github_issues: ["#34", "#35"]
---

# P30 — OpenAI 호환 백엔드 + sync --no-semantic (#34, #35)

## Description

두 가지 외부 이슈(batmania52)를 묶어 처리한다.

- **#35**: `graph semantic`에 LM Studio(OpenAI 호환) 백엔드 추가. 현재 `extract_with_ollama`는 `/api/chat`(Ollama 전용)을 사용하여 LM Studio `/v1/chat/completions`와 호환 불가.
- **#34**: `secall sync`에 `--no-semantic` 플래그 추가. sync.rs:293에서 `no_semantic`이 `false`로 하드코딩되어 크론 실행 시 GPU 메모리 경합 회피 불가.

## Expected Outcome

- `secall graph semantic --backend lmstudio --api-url http://localhost:1234 --model gemma-4-e4b-it` 동작
- `secall sync --no-semantic` 플래그로 시맨틱 추출 비활성화 가능
- 기존 ollama/gemini/anthropic/disabled 백엔드 영향 없음

## Subtasks

| # | Title | File | depends_on |
|---|-------|------|------------|
| 01 | OpenAI 호환 백엔드 함수 추가 | `p30-openai-sync-no-semantic-34-35-task-01.md` | — |
| 02 | sync --no-semantic 플래그 추가 | `p30-openai-sync-no-semantic-34-35-task-02.md` | — |

> Task 01, 02는 독립적이며 병렬 실행 가능 (parallel_group: A)

## Constraints

- `extract_with_openai_compat`는 `ollama_url` + `ollama_model` 설정을 재사용 (이슈 #35 제안)
- OpenAI 응답 파싱은 `choices[0].message.content` 경로

## Non-goals

- vLLM 등을 위한 범용 `openai_compatible` 백엔드명 (lmstudio만)
- wiki 백엔드에 lmstudio 추가 (#26과 별도)
- `--no-graph` 플래그 추가 (이슈 #34에서 "이상적"으로 언급했으나 별도 이슈로)
