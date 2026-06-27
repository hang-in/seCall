# Reference

seCall 의 현재 기준 사실(SSOT) 문서 인덱스. 모든 항목은 "지금 옳다고 합의된 상태" 이며, plans/prompts 와 달리 같은 파일을 갱신하는 것을 원칙으로 한다.

> 문서 규약(파일명, frontmatter, 인덱스 갱신 등)은 [docConventions.md](docConventions.md) 참고.

## 추천 읽기 순서

새 세션에서 컨텍스트를 파악할 때 다음 순서로 읽으면 충분하다.

1. 프로젝트 루트 [`CLAUDE.md`](../../CLAUDE.md) — 프로젝트 전체 규약과 현재 상태 요약
2. [handoff_2026-06-26.md](handoff_2026-06-26.md) — 최신 세션 핸드오프 (현재 작업 맥락)
3. [core-backlog.md](core-backlog.md) / [web-backlog.md](web-backlog.md) — 현 우선순위와 백로그
4. 작업 영역별 reference (LLM 설정이면 `llm-config.md`, Wiki 작업이면 `wiki-setup.md` 등)
5. (필요 시) 관련 [plans](../plans/index.md) / [prompts](../prompts/index.md)

## 문서 목록

### 프로젝트 기준

- [roadmap.md](roadmap.md) — seCall 전체 로드맵 (Phase 1~4, 아키텍처, 기술 스택) · 상태: done
- [docConventions.md](docConventions.md) — 문서 규약 (파일명/frontmatter/index/아카이브) · 상태: done
- [core-backlog.md](core-backlog.md) — 코어(Rust CLI) 백로그 및 현 우선순위 · 상태: in_progress
- [web-backlog.md](web-backlog.md) — Web/REST/Obsidian 플러그인 백로그 · 상태: in_progress

### 세션 핸드오프 (시간순 스냅샷, 최신만 우선 참고)

- [handoff_2026-06-26.md](handoff_2026-06-26.md) — 최신 세션 핸드오프 (PR #108 archive 리뷰 + 백로그 정리 PR #110; 다음=배포방법 고도화) · 상태: draft
- [handoff_2026-06-25.md](handoff_2026-06-25.md) — 이전 세션 핸드오프 (v0.6.0~0.6.3 릴리스 + 외부 PR + CLI 변동 점검) · 상태: done
- [handoff_2026-05-19.md](handoff_2026-05-19.md) — 이전 세션 핸드오프 (v0.5.0+10 PR) · 상태: done
- [handoff_2026-05-12.md](handoff_2026-05-12.md) — 이전 세션 핸드오프 · 상태: archived (직전 스냅샷, 보존)

### 운영·설정 가이드

- [llm-config.md](llm-config.md) — LLM 백엔드 설정 (Ollama/Gemini/Anthropic, env/config.toml 우선순위) · 상태: done
- [wiki-setup.md](wiki-setup.md) — Wiki 파이프라인 초기 설정 · 상태: done
- [github-vault-sync.md](github-vault-sync.md) — Obsidian Vault ↔ GitHub 동기화 절차 · 상태: done
- [daily-host-suffix-handoff.md](daily-host-suffix-handoff.md) — 데일리 노트 host suffix 처리 핸드오프 · 상태: done
- [sync-monitor-2026-05-15.md](sync-monitor-2026-05-15.md) — Sync 모니터링 운영 기록 · 상태: partial

### 설계·아이디어

- [adr-blocking-io-in-async.md](adr-blocking-io-in-async.md) — ADR: async 내 blocking I/O 는 `spawn_blocking` 으로 래핑 (CLI 특성상 정당) · 상태: done
- [idea-two-tier-llm-pipeline.md](idea-two-tier-llm-pipeline.md) — 아이디어: 2계층 LLM 파이프라인 (저비용 초안 → 고품질 검수) · 상태: draft, canonical: false
- [antigravityIngestFeasibility.md](antigravityIngestFeasibility.md) — Antigravity CLI 세션 ingest feasibility (비공개 protobuf schema → 현재 보류, 재개 트리거 명시) · 상태: done

## CLI Reference

### `secall graph semantic`

시맨틱 그래프 엣지 재추출 (임베딩 미포함).

| 플래그 | 설명 | 기본값 |
|--------|------|--------|
| `--delay <SECS>` | 세션 간 대기 시간 (소수점 가능) | 2.5 |
| `--limit <N>` | 최대 처리 세션 수 | 전체 |
| `--backend <NAME>` | LLM 백엔드 (`ollama`/`ollama_cloud`/`anthropic`/`lmstudio`/`disabled`) | config.toml |
| `--api-url <URL>` | API base URL (Ollama 전용) | config.toml |
| `--model <NAME>` | 모델명 (예: `gemma4:e4b`) | config.toml |
| `--api-key <KEY>` | API 키 (ollama_cloud용 → `cloud_api_key`). 환경변수 사용 권장 | config.toml |

**환경변수** (우선순위: CLI 플래그 > 환경변수 > config.toml > 기본값):

| 환경변수 | 용도 | 예시 값 |
|----------|------|---------|
| `SECALL_GRAPH_BACKEND` | 시맨틱 백엔드 | `ollama_cloud`, `ollama`, `disabled` |
| `SECALL_GRAPH_API_URL` | API base URL (Ollama용) | `http://localhost:11434` |
| `SECALL_GRAPH_MODEL` | 모델명 | `gemma4:e4b` |
| `OLLAMA_CLOUD_API_KEY` | ollama_cloud API 키 (graph/log/embedding 공용) | `...` |

> **참고**: ollama_cloud API 키는 `OLLAMA_CLOUD_API_KEY` 환경변수 또는 `--api-key` 플래그로 제공한다. anthropic 백엔드는 `ANTHROPIC_API_KEY` 를 직접 읽는다. (구 `SECALL_GEMINI_API_KEY`·`SECALL_GRAPH_API_KEY` 는 `apply_env_overrides` 에서 더 이상 읽지 않음 — gemini 백엔드 제거 잔재)
