---
type: task
status: draft
updated_at: 2026-05-05
plan_slug: p39-wiki-sync-auto-commit-fix
task_id: 04
parallel_group: C
depends_on: [00, 01, 02, 03]
---

# Task 04 — README + Insight findings + 컨트리뷰터 답변 초안

## Changed files

수정:
- `README.md` — changelog 표 상단에 v0.8.2 P39 행 추가 + (옵션) "운영" 또는 "vault sync" 섹션이 있다면 auto-commit fix 안내 한 줄.
- `README.en.md` — 영문판 동일.

신규:
- `docs/insight/findings/STA-vault-auto-commit-패턴-누락.md` — auto-commit 누락 사고 finding 등록 + Task 01 fix 결과로 status `resolved` (P38 Insight finding 형식 따라).
- `docs/community/p39-wiki-vector-response.md` — 외부 컨트리뷰터 댓글 답변 초안. Task 04 의 P40 결정 데이터 인용.

## Change description

### README changelog

```text
| 2026-XX-XX | v0.8.2 | P39 wiki 파이프라인 baseline + sync auto-commit fix: VaultGit::auto_commit 가 누락하던 SCHEMA.md / graph/ / log/ 등 모두 stage, 7+ 회귀 tests, 683 세션 sync baseline 측정 (docs/baseline/p39-*.md) |
```

날짜 placeholder, 머지 시점 갱신.

### Insight finding 신규 등록 형식

기존 finding 마크다운 bullet 형식 따라:
```text
# secall sync — vault auto-commit 패턴 누락

- **Category**: stability (STA)
- **Severity**: minor (재실행 가능 + graceful degradation)
- **Fix Difficulty**: easy
- **Status**: resolved
- **Resolved At**: 2026-XX-XX
- **Resolved By**: crates/secall-core/src/vault/git.rs:146 (P39 Task 00)
- **File**: crates/secall-core/src/vault/git.rs:146

## Description

`VaultGit::auto_commit` 가 명시 패턴 (`raw/ wiki/ index.md log.md .gitignore`) 만
stage 하여 SCHEMA.md / graph/ / log/ 등 누락. 매 sync 마다 일부 untracked
가 남아 git pull --rebase 가 "스테이징하지 않은 변경 사항" 으로 실패.

## Evidence

3회 누적 "auto: uncommitted vault changes" commit 후에도 vault 에
`M SCHEMA.md`, `?? graph/`, `?? log/` 잔존. 사용자 sync 진행 중 발견.

## Fix

`git add -A` (또는 누락 패턴 추가) + 회귀 테스트 7~8건.
```

### 외부 컨트리뷰터 답변 초안 (마크다운 + 보관)

```text
# Wiki 벡터화 답변 (외부 컨트리뷰터, 2026-05-05)

## 원 댓글
> "조금 다른 방향의 질문인데 secall recall은 벡터 방식이고 wiki_search는
>  bm25기반 검색인데 wiki 벡터화 계획인 있으신지요?"

## 답변 초안 (한국어, 한국어 댓글 톤)

[Task 03 의 측정 데이터 (페이지 수 / 평균 길이 / 검색 빈도) 인용]

[Task 03 의 결정 (P40 즉시 진행 / 보류) 반영]

[보류 시: 재측정 약속, 즉시 진행 시: P40 plan 진행 의사 + 시점 명시]

## 게시 결정 (사용자)

- [ ] 게시 OK — GitHub 댓글에 위 답변 그대로
- [ ] 수정 필요 — ...

## 게시 후 메모
- 게시 일시: ...
- 후속 응답: ...
```

답변 본문은 Task 03 결과 들어온 후 채움 — 본 task 에서는 placeholder + 구조만.

### 사용자 vault 정리 안내 (README 또는 별도 메모)

본 phase 에서 fix 한 auto-commit 코드는 **다음 sync 부터** 적용. 현재 사용자 vault 의 잔존 unstaged 는 사용자가 수동 정리:

```text
cd <vault.path>
git status --short
git add -A
git commit -m "manual: post-P39 backfill"
git pull --rebase origin main
git push
```

위 안내를 README "Troubleshooting" 또는 별도 `docs/operations/p39-vault-cleanup.md` 에 보관 — 사용자 결정.

### CI 변경 없음

기존 cargo test job 이 Task 00 신규 vault_auto_commit 테스트 자동 실행. workflow 수정 불필요.

## Dependencies

- 외부: 없음
- 내부 task: Task 00 (fix 결과 finding status), Task 01 (baseline 데이터), Task 02 (품질 데이터), Task 03 (P40 결정 데이터) 모두 완료 후

## Verification

```bash
grep -qE "P39|wiki baseline|auto-commit fix" /Users/d9ng/privateProject/seCall/README.md && echo "ko P39 OK"
grep -qE "P39|wiki baseline|auto-commit fix" /Users/d9ng/privateProject/seCall/README.en.md && echo "en P39 OK"
ls -la /Users/d9ng/privateProject/seCall/docs/insight/findings/STA-vault-auto-commit-패턴-누락.md
ls -la /Users/d9ng/privateProject/seCall/docs/community/p39-wiki-vector-response.md
git diff --stat .github/workflows/ | head -3
```

수동: 답변 초안에 Task 03 데이터 정확히 인용됐는지 검토.

## Risks

- **README 일관성**: 정확한 test 카운트 / 보고서 파일명 / 결정 결과를 README 에 적시 → Task 00-03 검증 통과 + 실측 후 본 task 진행.
- **답변 초안 톤**: 외부 댓글 답변은 사용자가 게시 → 톤이 너무 기술적이거나 방어적이면 부적절. placeholder 단계에서 톤 가이드 (친절 + 정확 + 짧게) 명시.
- **finding 형식 불일치**: 기존 finding markdown bullet 형식 (P38 처리 패턴 그대로 — frontmatter YAML 아님).
- **vault 정리 안내 위치**: README 의 본문 흐름과 무관할 수 있음 — Troubleshooting 섹션 없으면 별도 docs/operations/ 권장.

## Scope boundary

수정 금지:
- `crates/`, `web/src/` 코드 — Task 00-03 완료 후 본 task 는 문서만
- `.github/workflows/*` — 변경 없음
- 본 task 외 Insight findings (TES-* / 다른 STA-*) — 무관
- 다른 phase 의 plan 문서
- 외부 컨트리뷰터 댓글 직접 게시 (사용자 검토 후 직접 게시)
