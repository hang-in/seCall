---
type: plan
status: draft
updated_at: 2026-04-06
version: 1
---

# seCall P5 — Vault Git Sync + Reindex

## Description

멀티기기 사용자를 위한 vault 동기화 인프라 구축.
Git 기반 vault push/pull로 마크다운 세션을 기기 간 공유하고, reindex로 동기화된 MD를 로컬 DB에 인덱싱.
`secall sync` 명령 하나로 전체 흐름(git pull → reindex → ingest → git push) 자동화.

DB는 파생 캐시로 취급 — MD가 source of truth이며, DB 손실 시 vault에서 완전 복구 가능.

## Expected Outcome

- 각 기기에서 `secall sync` 실행 시 git pull → reindex → ingest → git push 자동 수행
- 다른 기기에서 생성된 세션이 MCP recall로 검색 가능
- DB 손실 시 `secall reindex --from-vault`로 완전 복구
- 세션 frontmatter에 `host` 필드로 생성 기기 추적
- GitHub 연동 설정 안내 문서 제공

## Subtasks

| # | Title | 공수 | parallel_group | depends_on |
|---|-------|------|---------------|------------|
| 01 | MD → DB 역인덱싱 (reindex --from-vault) | Medium | A | — |
| 02 | Vault Git 연동 (init/pull/push) | Small | A | — |
| 03 | `secall sync` 통합 명령 | Small | B | 01, 02 |
| 04 | host 필드 추가 | Small | A | — |

## Constraints

- P3/P4 완료 후 진행 (CI 안전망 + typed error 기반)
- 기존 vault 구조 (raw/sessions/, wiki/) 유지 — 하위 호환
- Git 미설정 시에도 기존 기능 정상 동작 (git은 선택적)
- reindex는 기존 세션 중복 skip (session_id 기준)

## Non-goals

- Claude Desktop 세션 지원 — export API 미공개 상태
- 실시간 동기화 (fswatch/watchman) — cron 또는 hook으로 충분
- TUI 대시보드 — P6으로 분리
- Obsidian 플러그인 — P7 이후
- 충돌 해결 로직 — 세션은 기기별 유니크하므로 충돌 불가
