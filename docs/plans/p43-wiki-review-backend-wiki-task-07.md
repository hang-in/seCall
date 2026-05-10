---
type: task
plan_slug: p43-wiki-review-backend-wiki
task_id: 07
title: Documentation
parallel_group: D
depends_on: [01, 02, 03, 04, 05, 06]
status: pending
updated_at: 2026-05-09
---

# Task 07 — Documentation

## Changed files

수정:
- `README.md` (한국어) — Wiki review 섹션 + Available Keys 표 + Updates 표 갱신.
- `README.en.md` (영문) — 동일 변경 영문 미러링.
- `docs/prompts/wiki-review.md` — task 04 의 본문 추가는 task 04 가 이미 완료 — 본 task 는 cross-backend 가이드 한 절 추가 (어떤 backend 에서 어떤 issue 자주 나는지).
- `docs/community/v0.5.0-release-notes.md` (또는 신규 파일) — P43 변경 highlight.

신규:
- `docs/reference/llm-config.md` (신규) — P41 task 06 미생성분. 모든 LLM 설정 옵션 + default + 환경변수 + CLI/REST/Web/wiki review 진입점 한 곳에 정리.

## Change description

### 1. README — Wiki review 섹션

```markdown
### Wiki review (다중 backend)

`secall wiki update <session> --review` 가 ANTHROPIC_API_KEY 없는 환경에서도 동작합니다 (P43).

| Backend | 인증 | JSON 신뢰성 | 비용 |
|---|---|---|---|
| anthropic (default) | ANTHROPIC_API_KEY | 높음 (native JSON) | API 과금 |
| haiku | ANTHROPIC_API_KEY | 높음 | API 과금 |
| claude | claude CLI | 중간 (prompt 의존) | 무료 (subscription) |
| codex | codex CLI | 중간 | 무료 (subscription) |
| ollama | 없음 (local) | 모델별 차이 | 무료 (local) |
| lmstudio | 없음 (local) | 모델별 차이 | 무료 (local) |

선택 우선순위:
1. CLI flag: `--review-backend ollama`
2. Config: `[wiki].review_backend = "ollama"`
3. Fallback: `[wiki].default_backend` (5종 중 하나면 그대로, 아니면 "haiku")

설정 예:
\`\`\`bash
secall config set wiki.review_backend ollama
secall config set wiki.review_model gemma4:e4b
\`\`\`

review 의 출력은 valid JSON object 만 — local backend 는 프롬프트가 strict-JSON suffix 를 자동 inject 합니다 (`docs/prompts/wiki-review-strict-json.md`).
```

### 2. README Available Keys 표

```
| 키 | 설명 | default |
|---|---|---|
| `wiki.review_backend` | review backend 이름 (claude/codex/haiku/ollama/lmstudio/anthropic) | `[wiki].default_backend` 폴백 |
| `wiki.review_model` | review 모델 (anthropic/haiku 만 의미 있음) | `sonnet` |
| ... |
```

### 3. README Updates 표 (한/영)

```
| 2026-05-?? | v0.10.0 | Wiki review 다중 backend (P43): 5 backend 지원 (claude/codex/haiku/ollama/lmstudio),
                       `[wiki].review_backend` config + `--review-backend` CLI, toml_edit 도입으로 사용자 주석 보존,
                       P42 review recommendations 정리 (가시성 축소, test_backend 분할, rest_config 정규식). |
```

### 4. docs/reference/llm-config.md 신규

```markdown
---
type: reference
title: LLM Configuration Reference
status: in_progress
updated_at: 2026-05-09
---

# LLM Configuration Reference

seCall 의 LLM 관련 모든 설정 옵션 + default + 환경변수 + 진입점.

## 카테고리 매트릭스

| 카테고리 | 명령 | 영향 받는 키 |
|---|---|---|
| Wiki 생성 | `secall wiki update` | `[wiki]` 전체 |
| Wiki review | `secall wiki update --review` | `[wiki].review_backend`, `[wiki].review_model` |
| Daily diary | `secall log` | `[log]` 전체 + `[graph].semantic_backend` 폴백 |
| Semantic graph | `secall graph rebuild` / `--semantic` | `[graph]` 전체 |
| Embedding | `secall sync` | `[embedding]` 전체 |

## Backend 매트릭스

| Backend | Wiki gen | Wiki review | Daily | Graph | Embedding |
|---|---|---|---|---|---|
| claude (CLI) | ✅ | ✅ (P43) | ✅ | — | — |
| codex (CLI) | ✅ | ✅ (P43) | ✅ | — | — |
| haiku (API) | ✅ | ✅ (P43) | ✅ | — | — |
| anthropic (API, sonnet/opus) | — | ✅ (default) | — | ✅ | — |
| ollama (local) | ✅ | ✅ (P43) | ✅ | ✅ | ✅ |
| lmstudio (local) | ✅ | ✅ (P43) | ✅ | ✅ | — |
| gemini (API) | — | — | ✅ | ✅ | — |
| ort / openvino | — | — | — | — | ✅ |

## 환경변수

| Var | 사용처 | 필수? |
|---|---|---|
| `ANTHROPIC_API_KEY` | haiku / anthropic backend | API 사용 시 |
| `SECALL_GEMINI_API_KEY` | gemini backend | API 사용 시 |
| `OPENAI_API_KEY` | (예약) | — |
| `SECALL_CONFIG_PATH` | config.toml 위치 override | 테스트 시 |
| `SECALL_WIKI_REVIEW_PROMPT` | review prompt 외부 path override (P43) | — |

## 진입점

| 진입점 | 변경 가능한 키 |
|---|---|
| CLI: `secall config set <key> <value>` | 모든 키 |
| CLI: `secall config llm test [<backend>]` | 검증 (read-only) |
| REST: `GET /api/config` | 조회 (sanitized) |
| REST: `PATCH /api/config/{wiki|graph|log|embedding}` | 섹션별 (`--allow-config-edit` 필요) |
| Web: `/settings` | 4 카테고리 form |
| 직접 편집: `~/.config/secall/config.toml` | 모든 키 (P43 toml_edit 으로 주석 보존) |

## Default 값

| 키 | Default |
|---|---|
| `[wiki].default_backend` | `"ollama"` |
| `[wiki].review_backend` | `null` → `default_backend` 폴백 |
| `[wiki].review_model` | `"sonnet"` |
| `[graph].semantic_backend` | `"ollama"` |
| `[graph].ollama_model` | `"gemma4:e4b"` |
| `[graph].gemini_model` | `"gemini-2.5-flash"` |
| `[graph].anthropic_model` | `"claude-haiku-4-5-20251001"` |
| `[log].backend` | `null` → `[graph].semantic_backend` 폴백 |
| `[embedding].backend` | `"ollama"` |
| `[embedding].ollama_model` | `"bge-m3"` |

## Troubleshooting

### `wiki update --review` 실패: "ANTHROPIC_API_KEY not set"

P43 이전: API key 없으면 review 자체 불가.
P43 이후: `secall config set wiki.review_backend ollama` 로 로컬 backend 전환.

### Local backend review 가 JSON parse 실패

ollama / lmstudio 가 markdown 으로 wrapping 시 1회 자동 retry. 2회 실패 시:
- 모델 변경 (gemma3 / qwen2.5 권장 — JSON 신뢰성 높음)
- `docs/prompts/wiki-review-strict-json.md` 의 prompt 강화

### `secall config set` 후 사용자 주석 사라짐

P43 이전: toml::to_string_pretty 는 주석 미보존.
P43 이후: toml_edit 도입으로 주석 / 공백 / 키 순서 보존.
```

### 5. release-notes (신규 또는 갱신)

```markdown
## v0.10.0 (P43)

- Wiki review 가 5 backend 지원 (P43). 로컬 전용 환경 (ollama / lmstudio) 에서도 `wiki update --review` 동작.
- toml_edit 도입 — `secall config set` / Web Settings 에서 변경해도 사용자 주석 보존.
- P42 review recommendations 정리 (semantic.rs 가시성 축소, test_backend 함수 분할).
- `docs/reference/llm-config.md` 신규 — 모든 LLM 옵션 한 곳에 정리.
```

## Dependencies

- task 01–06 모두 완료 필요 — 본 task 는 그 결과물을 docs 에 반영.
- npm/cargo dep: 추가 없음.

## Verification

```bash
# 1. README 변경 grep
grep -c "review_backend" README.md       # 1 이상
grep -c "review_backend" README.en.md    # 1 이상
grep "v0.10.0" README.md                 # Updates 표

# 2. llm-config.md 존재 + 핵심 섹션
ls docs/reference/llm-config.md
grep -c "Backend 매트릭스" docs/reference/llm-config.md
grep -c "환경변수" docs/reference/llm-config.md
grep -c "Troubleshooting" docs/reference/llm-config.md

# 3. (수동) markdown lint (있으면)
markdownlint README.md README.en.md docs/reference/llm-config.md

# 4. 한/영 일관성 — Updates 표의 v0.10.0 line
diff <(grep "P43" README.md | head -1) <(grep "P43" README.en.md | head -1) || true
# (한국어/영문 차이는 의도 — 내용만 동일하면 OK)

# 5. (수동) docs link 무결성
grep -E "\(docs/.+\.md\)" README.md | while read line; do
  path=$(echo "$line" | sed -E 's/.*\((docs\/[^)]+)\).*/\1/')
  test -f "$path" || echo "MISSING: $path"
done
```

## Risks

- **doc-code drift** — task 01–06 의 결과를 정확히 반영해야 함. drift 시 사용자가 README 보고 명령 실행해서 fail. mitigation: review 시 task 03 의 CLI flag 이름과 README 의 example 이 일치하는지 cross-check.
- **번역 누락** — 한국어 / 영문 둘 다 갱신해야. diff verification 으로 큰 갭 검출.
- **release notes vs README Updates** — 둘 다 갱신.
- **`docs/reference/llm-config.md` 의 default 값 drift** — 본 plan 이 task 02 의 `defaults.rs` constants 를 doc 에 hard-code. constants 변경 시 doc 도 갱신 — 향후 별도 lint script 가능.
- **task 06 의 semantic_backends.rs 삭제** 가 README 의 회귀 테스트 안내에 영향 — README 의 `cargo test --test semantic_backends` 같은 라인이 있다면 갱신 필요.

## Scope boundary (수정 금지)

- 코드 (`crates/`, `web/src/`) — 본 task 는 docs only.
- `docs/plans/p43-*.md` — 이 plan 의 다른 task 문서. 본 task 가 그 문서를 손대지 않음.
- 다른 plan 의 docs (`docs/plans/p4X-*` 외) — 영역 외.
- `docs/prompts/wiki-review-strict-json.md` — task 04 영역 (생성).
- `docs/prompts/wiki-review.md` 의 본문 추가 부분 — task 04 영역. 본 task 는 cross-backend 가이드 한 절만 추가.
