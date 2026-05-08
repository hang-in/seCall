---
type: plan
slug: p41-llm-daily-diary-web
status: drafting
updated_at: 2026-05-08
---

# P41 — LLM 설정 통합 + Daily diary 다중 백엔드 + Web 설정 화면

## Description

secall 의 LLM 사용은 4 카테고리 (wiki / wiki-review / graph-semantic / log-diary + embedding) 다. 현재 다음 한계가 있다:

1. **Daily diary 가 ollama 전용** — `crates/secall/src/commands/log.rs:123` 의 분기가 `config.graph.semantic_backend == "ollama"` 일 때만 LLM 호출. claude / codex / haiku / gemini 등 다른 백엔드 사용 불가.
2. **모델 default 가 코드에 hard-coded** — `gemma4:e4b` (log.rs:129, semantic.rs:419), `gemini-2.5-flash` (semantic.rs:284), `gemma-4-e4b-it` (semantic.rs:435), `sonnet` (wiki.rs:562), `gpt-5.4` (wiki.rs:563) 가 config 미설정 시 fallback. config 노출 안 됨 → 사용자 입장에서 어떤 default 가 동작할지 불투명.
3. **웹에서 설정 못 함** — config.toml 직접 편집 필요. `secall serve` 가 config 노출/편집 endpoint 없음. Web UI 의 `/settings` 라우트 없음.

본 plan 의 목표는 위 세 한계 해소.

## Expected Outcome

- `secall log --backend claude|codex|haiku|ollama|lmstudio` 로 일기 백엔드 선택 가능. `[log]` config 섹션 신규.
- 모든 hard-coded model default 가 config 의 명시적 필드로 노출. config 미설정 시 코드의 fallback 은 그대로 두되 `tracing::warn!` 으로 안내.
- `GET /api/config` (sanitized — secret 마스킹) + `PATCH /api/config/<section>` (선택적, default-disabled 보안 모드).
- `/settings` 라우트 (web) — Wiki / Graph / Log / Embedding 4 카테고리 form, 저장 시 config.toml 갱신.
- `secall config` CLI 강화 — `secall config llm show` / `secall config llm set <key> <value>` (기존 `run_show`/`run_set` 확장).
- README Configuration 섹션 + Available Keys 표 갱신.

## Subtask Summary

| # | 제목 | parallel_group | depends_on |
|---|---|---|---|
| 01 | Daily diary 다중 백엔드 (`secall log --backend`) | A | — |
| 02 | 하드코딩된 모델 default config 노출 | A | — |
| 03 | REST `/api/config` (read + 선택적 write) | B | 01, 02 |
| 04 | Web Settings 라우트 + form | C | 03 |
| 05 | CLI `secall config` 강화 | A | 02 |
| 06 | README + design-tokens.md 후속 갱신 | D | 01–05 |

병렬 가능: A 그룹 (01 / 02 / 05) — 같은 config.rs 영역 충돌 가능하니 02 먼저 → 01·05 순서 권장.

## Constraints

- **호환성**: 기존 `config.toml` 의 모든 키 유지. 새 필드는 `#[serde(default)]` + 누락 시 `tracing::warn!`.
- **보안**: `secall serve` 의 config write endpoint 는 default-disabled. `--allow-config-edit` (또는 `[serve].allow_config_edit = true`) 활성화 필요. API key 같은 secret 은 web UI 에서 절대 노출 X — 마스킹 표시만.
- **기존 PR 호환**: 본 plan 은 PR #54 (web redesign) **머지 후** 시작. main 의 새 design tokens / TopNav / Layout 위에서 작업.
- **tunaFlow rule**: result.md 작성 task 추가 X (자동 생성).

## Risks

- **`graph.semantic_backend` 와 `log.backend` 의 의미 분리** — 현재 log 가 graph 의 semantic_backend 를 재사용하는데, 둘은 사실 별개 책임 (그래프 = 시맨틱 엣지 추출, log = 일기 생성). 분리 시 기존 `[graph]` 만 설정한 사용자가 회귀 없도록 fallback 체인 (`[log].backend → [graph].semantic_backend → "ollama"`) 필요.
- **CLI 백엔드 외부 의존** — `claude` / `codex` / `gemini-cli` 백엔드는 외부 CLI 가 PATH 에 있어야 동작. secall 이 깔린 환경에 cli 가 없으면 친절한 에러 (`command not found: claude. claude code 를 먼저 설치하세요`).
- **REST config write 의 toml 보존** — `vault/config.rs::save()` 가 사용자 주석/공백을 잃을 수 있음 (toml crate 의 기본 직렬화). 해결: `toml_edit` crate 도입 또는 read-only 권장.
- **Web Settings form 의 race** — 사용자가 form 편집 중 외부에서 config.toml 변경 시 마지막 쓰기 우선. 본 plan 에서는 단순히 마지막 PATCH 가 win (단일 사용자 가정).

## Scope boundary (수정 금지)

- `crates/secall-core/src/wiki/{claude,codex,haiku,ollama,lmstudio}.rs` — wiki backend 본체. log 가 이걸 **재사용** 만 하고 본체 변경 X.
- `crates/secall-core/src/search/embedding.rs` — embedding 백엔드. 본 plan 은 LLM (텍스트 생성) 만 다루고 embedding 은 손대지 않음.
- `crates/secall-core/src/store/` — DB 스키마 변경 없음.
- 직전 PR #54 의 `web/src/components/{TopNav,SearchBar,SessionList,...}` 등 — task 04 에서 Settings 라우트 추가 + TopNav 의 우상단에 톱니 1개 추가만, 다른 컴포넌트 시각 변경 X.

## Non-goals

- LLM 응답 비교/벤치마크 도구
- 모델 자동 선택 (auto-routing)
- `secall serve` 의 multi-user / 인증 — 로컬 단일 사용자 가정
- gemini-cli 백엔드의 신규 CLI 통합 — codex/claude 와 동일 패턴이면 task 01 에 포함, 아니면 본 plan 에서 제외.

## Versioning

이 plan 의 변경분은 별도 brand 머지 후 `v0.5.0` minor bump. main HEAD = PR #54 머지 결과 기준.
