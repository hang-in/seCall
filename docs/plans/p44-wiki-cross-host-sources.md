---
type: plan
slug: p44-wiki-cross-host-sources
title: P44 — Wiki cross-host 머지 (sources 합집합 재생성)
status: in_progress
updated_at: 2026-05-10
---

# P44 — Wiki cross-host 머지 (sources 합집합 재생성)

## Description

같은 vault 를 윈도우/맥에서 양방향 사용 시, 같은 토픽의 wiki 페이지가 양쪽 머신에서 갱신되어 git merge 충돌이 발생함. 사용자는 위키를 수동 편집하지 않으므로(메모리: `user_wiki_edit_policy`), frontmatter 의 `sources: Vec<String>` 합집합 기반 재생성으로 충돌을 자동 해소함.

동시에 동일 host 에서 같은 토픽을 두 번 호출했을 때 `merge_with_existing()` 가 본문을 단순 concat (`기존 + 구분선 + 새 내용`) 하여 본문이 누적되는 문제도 같이 정리함.

## Expected Outcome

- 윈도우 → `wiki update` → push → 맥 → `wiki update` → 자동 pull → 같은 토픽이 양쪽에서 만들어졌어도 conflict 0, 본문 단일 결과.
- 같은 host 에서 같은 토픽을 두 번 실행해도 본문 중복 누적 없음 (sources 만 합집합 유지).
- `--no-pull` flag 로 오프라인 환경 호환.

## Subtask Summary

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 01 | `secall wiki update` 진입 시 `auto_commit + pull` 자동 호출 | A | — |
| 02 | `merge_with_existing()` 본문 concat 제거 (sources 만 합집합) | A | — |
| 03 | Pull 후 wiki conflict marker 감지 → sources 합집합 재생성 자동화 | B | 01, 02 |
| 04 | 회귀 테스트 + 문서 (README + llm-config.md 미수정) | C | 01, 02, 03 |

## Constraints

- 수동 편집 보존 마커 불필요 (사용자 확인 — 위키는 수동 편집 안 함).
- 일기는 영역 외 — host suffix 회피 패턴 그대로 둠.
- 오프라인 환경 호환: `--no-pull` flag 제공.
- LLM 재호출 비용: cross-host conflict 발생 시에만 — 평소엔 추가 비용 0.

## Non-goals

- 일기(daily log) 머지 자동화 — 별도 plan.
- 위키 페이지의 history timeline / version UI.
- 사용자 편집 영역 보존 마커 (`<!-- user:start -->` 등).
- git conflict 가 wiki 외 파일 (raw/sessions/, log/, graph/) 에서 발생한 경우 자동 resolve — 본 plan 은 `wiki/*.md` 만.
- `--no-pull` 외 push 단계 자동화 — push 는 `secall sync` 의 영역.

## Plan version

v1.0 (2026-05-10) — 최초 작성.
