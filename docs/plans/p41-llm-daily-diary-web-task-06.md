---
type: task
plan_slug: p41-llm-daily-diary-web
task_id: 06
title: README + design-tokens.md 후속 갱신
parallel_group: D
depends_on: [01, 02, 03, 04, 05]
status: pending
updated_at: 2026-05-08
---

# Task 06 — README + design-tokens.md 후속 갱신

## Changed files

수정:
- `README.md` (한국어) — Configuration / 사용법 / 설정 키 목록 / Updates 섹션 갱신.
- `README.en.md` (영문) — 동일 변경 영문 미러링.
- `web/src/lib/design-tokens.md` — `/settings` 라우트 form 시각 가이드 추가 (input/select/switch 의 토큰 매핑).
- `docs/community/v0.4.0-release-notes.md` (또는 신규 v0.5.0 release notes) — P41 변경 highlight 추가.

신규:
- `docs/reference/llm-config.md` (선택, 단일 reference 문서) — 모든 LLM 설정 옵션 + default + 환경변수 + CLI/REST/Web 진입점 한 곳에 정리. 사용자 진입 문서.

## Change description

### 1. README 의 Configuration 섹션 갱신

추가/수정:
- `[log]` 섹션 신규 — `default_backend` / `model` / `api_url` / `max_tokens`
- `[graph]` 의 모든 모델 default 명시 (task 02 의 doc comment 와 일관)
- `secall log --backend` flag 사용 예
- `secall config llm show` / `secall config set log.backend haiku` CLI 예
- `secall serve --allow-config-edit` 의 `/settings` 라우트 안내 + 보안 경고

### 2. Available Keys 표 갱신

| 키 | 설명 | default |
|---|---|---|
| `log.backend` | Daily diary 백엔드 (claude / codex / haiku / ollama / lmstudio) | `[graph].semantic_backend` 폴백 |
| `log.model` | 백엔드의 모델 override | backend default |
| `wiki.backends.<name>.model` | 백엔드별 모델 | `gemma4:e4b` 등 |
| `graph.gemini_model` | Gemini 모델 | `gemini-2.5-flash` |
| ... |

### 3. Updates 섹션 (한/영 둘 다)

```
| 2026-05-?? | v0.5.0 | LLM 설정 통합 (P41): Daily diary backend 5종, 모델 default config 노출,
                     `/api/config` REST + `/settings` web 페이지, `secall config llm` CLI |
```

### 4. design-tokens.md 갱신

`/settings` 의 form input/select/switch 가 어떤 토큰을 사용하는지 짧게 추가:

```
input:        bg-[var(--surface)] border-border-soft focus:ring-2 focus:ring-brand-soft focus:border-brand
select:       (위와 동일 + chevron icon text-text-3)
switch:       data-[state=checked]:bg-brand
disabled:     opacity-50 cursor-not-allowed
masked input: text-text-4 placeholder:text-text-4 italic
```

### 5. (선택) `docs/reference/llm-config.md`

단일 reference 문서로:
- 4 카테고리 표 (어떤 명령이 어떤 설정 사용)
- 환경변수 list (마스킹 / set 여부 만 노출)
- CLI / REST / Web 진입점 매핑
- 흔한 troubleshooting (e.g., "`secall log` 가 동작 안 함" → ollama 실행 확인 + backend 옵션)

## Dependencies

- **task 01–05 모두 완료 필요** — 본 task 는 그 결과물을 docs 에 반영.
- npm/cargo dep: 추가 없음.

## Verification

```bash
# README 마크다운 lint (있으면)
markdownlint README.md README.en.md

# (수동) README 의 새 섹션 직접 확인
grep -A3 "log\." README.md       # [log] 섹션 본문
grep "config llm"  README.md       # CLI 예
grep "/settings"   README.md       # web 안내
grep "secall log"  README.md       # backend flag 예

# 한/영 수치 일관성
diff <(grep -E "^\| [0-9-]" README.md | head -5) <(grep -E "^\| [0-9-]" README.en.md | head -5)
```

## Risks

- **doc 과 코드 drift** — README 가 task 01–05 의 결과를 반영해야 하는데, 그 사이 default 값이 바뀌면 doc 이 stale. fix: 모든 default 를 `crates/secall-core/src/llm/defaults.rs` (task 02) 의 constants 를 참조하도록 doc 작성 + 향후 변경 시 task 02 의 회귀 테스트가 trip.
- **번역 누락** — 한국어/영어 둘 다 갱신해야. diff verification 으로 잡음.
- **release notes vs README Updates** — release notes 는 GitHub Release 본문, README Updates 는 repo 안. 둘 다 업데이트 필요.

## Scope boundary (수정 금지)

- 코드 (`crates/`, `web/src/`) — 본 task 는 docs only.
- `docs/plans/p41-*.md` — 이 plan 의 다른 task 문서. 본 task 가 그 문서를 손대지 않음.
- 다른 plan 의 docs (`docs/plans/p3X-*` 등) — 영역 외.
