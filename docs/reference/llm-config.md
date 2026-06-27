---
type: reference
title: LLM Configuration Reference
status: active
updated_at: 2026-05-12
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
| `ollama_cloud` | — | — | ✅ | ✅ | ✅ |
| `ort` / `openvino` | — | — | — | — | ✅ |

## 환경변수

| Var | Usage |
|---|---|
| `ANTHROPIC_API_KEY` | `anthropic` / `haiku` API backend |
| `OLLAMA_CLOUD_API_KEY` | `ollama_cloud` graph / log / **embedding** backend |
| `SECALL_CONFIG_PATH` | 테스트/로컬 config path override |
| `SECALL_WIKI_REVIEW_PROMPT` | wiki review prompt override |

## Config Keys

| Key | Default | Notes |
|---|---|---|
| `[wiki].default_backend` | `"codex"` | wiki generation 기본 backend |
| `[wiki].review_backend` | `null` | 없으면 `default_backend`, 불명확하면 `haiku` |
| `[wiki].review_model` | `"sonnet"` | anthropic 계열 review model override |
| `[graph].semantic_backend` | `"ollama"` | semantic graph backend |
| `[graph].ollama_model` | `gemma4:e4b` | ollama / lmstudio semantic model |
| `[graph].cloud_host` | `https://ollama.com` | ollama_cloud API 호스트 |
| `[graph].cloud_model` | `gemma4:31b-cloud` | ollama_cloud semantic model |
| `[graph].anthropic_model` | `claude-haiku-4-5-20251001` | anthropic graph model |
| `[log].backend` | `null` | 없으면 `graph.semantic_backend` |
| `[log].cloud_host` | `https://ollama.com` | log ollama_cloud 호스트 override |
| `[log].cloud_model` | `kimi-k2.6:cloud` | log ollama_cloud model override |
| `[embedding].backend` | `"ollama"` | embedding backend |
| `[embedding].pool_size` | `null` (auto) | ORT session pool size. 미설정 시 시스템 RAM 기반 자동 결정 (≤15GB→1, 16-31GB→2, ≥32GB→4) |
| `[embedding].cloud_host` | `https://ollama.com` | ollama_cloud embedding API 호스트 |
| `[embedding].cloud_model` | `null` | ollama_cloud embedding 모델 이름 |

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

## P46 마이그레이션 (Gemini → Ollama Cloud)

기존 사용자가 `[graph] semantic_backend = "gemini"` 또는 `[log] backend = "gemini"` 를 쓰고 있었다면:

1. `config.toml` 의 `[graph]` 섹션에서 `gemini_api_key`, `gemini_model` 줄 제거
2. `semantic_backend = "ollama_cloud"` 로 변경
3. `.env` 에 `OLLAMA_CLOUD_API_KEY=<key>` 설정
4. graph 와 log 가 다른 모델을 쓰려면 `[graph].cloud_model` 과 `[log].cloud_model` 분리 설정

기존 `.env` 의 `SECALL_GEMINI_API_KEY` 는 참조 코드가 제거되었으므로 삭제해도 되지만 남겨둬도 무방합니다.

## Apple Silicon (M1/M2/M3/M4) 가속 빌드

macOS aarch64 환경에서 ORT 임베딩 백엔드를 사용한다면 CoreML EP 를 활성화해 ANE / GPU 가속을 사용할 수 있습니다.

```bash
cargo build --release -p secall --features secall-core/coreml
```

빌드 후 stderr 로그의 `ORT session pool created coreml=true` 로 EP 활성 여부를 확인할 수 있습니다. CoreML 등록이 실패하면 ORT 가 자동으로 CPU 로 폴백합니다.

## P47 마이그레이션 (임베딩 부담 완화 — M4 Air 16GB)

- macOS aarch64 + ORT 백엔드 사용 시 `--features secall-core/coreml` 빌드로 ANE/GPU 가속.
- 16GB 환경에선 `[embedding] pool_size = 1` 권장 (default 자동 결정).
- 임베딩 자체를 Cloud 로 옮기려면 `[embedding] backend = "ollama_cloud"` + `OLLAMA_CLOUD_API_KEY` 설정.
