---
type: reference
title: LLM Configuration Reference
status: active
updated_at: 2026-05-09
---

# LLM Configuration Reference

seCall 의 LLM 관련 설정, 환경변수, 진입점을 한 곳에 정리한 문서입니다.

## Backend 매트릭스

| Backend | Wiki generation | Wiki review | Daily log | Semantic graph | Embedding |
|---|---|---|---|---|---|
| `claude` | ✅ | ✅ | ✅ | — | — |
| `codex` | ✅ | ✅ | ✅ | — | — |
| `haiku` | ✅ | ✅ | ✅ | — | — |
| `anthropic` | — | ✅ | — | ✅ | — |
| `ollama` | ✅ | ✅ | ✅ | ✅ | ✅ |
| `lmstudio` | ✅ | ✅ | ✅ | ✅ | — |
| `gemini` | — | — | ✅ | ✅ | — |
| `ort` / `openvino` | — | — | — | — | ✅ |

## 환경변수

| Var | Usage |
|---|---|
| `ANTHROPIC_API_KEY` | `anthropic` / `haiku` API backend |
| `SECALL_GEMINI_API_KEY` | `gemini` graph or log backend |
| `SECALL_CONFIG_PATH` | 테스트/로컬 config path override |
| `SECALL_WIKI_REVIEW_PROMPT` | wiki review prompt override |

## Config Keys

| Key | Default | Notes |
|---|---|---|
| `[wiki].default_backend` | `"claude"` | wiki generation 기본 backend |
| `[wiki].review_backend` | `null` | 없으면 `default_backend`, 불명확하면 `haiku` |
| `[wiki].review_model` | `"sonnet"` | anthropic 계열 review model override |
| `[graph].semantic_backend` | `"ollama"` | semantic graph backend |
| `[graph].ollama_model` | `gemma4:e4b` | ollama / lmstudio semantic model |
| `[graph].gemini_model` | `gemini-2.5-flash` | gemini graph model |
| `[graph].anthropic_model` | `claude-haiku-4-5-20251001` | anthropic graph model |
| `[log].backend` | `null` | 없으면 `graph.semantic_backend` |
| `[embedding].backend` | `"ollama"` | embedding backend |

## Entry Points

| Entry point | Description |
|---|---|
| `secall config set <key> <value>` | 모든 설정 변경 |
| `secall config llm test [backend]` | backend 상태 점검 |
| `secall wiki update --review --review-backend <name>` | wiki review backend 지정 |
| `GET /api/config` / `PATCH /api/config/{section}` | REST config 조회/수정 |
| `/settings` | web UI config 편집 |

## Troubleshooting

- `wiki update --review` 에서 `ANTHROPIC_API_KEY not set` 가 나오면 `wiki.review_backend=ollama` 또는 `lmstudio` 로 전환합니다.
- 로컬 backend review 가 JSON 파싱에 실패하면 strict JSON suffix 로 1회 자동 재시도합니다.
- `secall config set` 후 주석이 사라지던 문제는 P43 에서 `toml_edit` 저장으로 해결되었습니다.
