---
type: plan
slug: p45-session-lifecycle-backbone-archive-vault-ssot-filter
title: P45 — Session lifecycle backbone (archive + vault SSOT + 기본 filter)
status: in_progress
updated_at: 2026-05-12
---

# P45 — Session lifecycle backbone (archive + vault SSOT + 기본 filter)

## Description

세션을 안전하게 숨길 수 있는 **archive/restore 기본 골격**을 vault SSOT 패턴 + DB 캐시 구조로 구축한다. 이번 plan 은 P46~P49 (REST + Web + classifier + prune + wiki/graph filter) 의 기반.

핵심 결정:

- **vault raw session markdown frontmatter 가 SSOT** — `archived: true, archived_at: ...` 직접 기록. cross-host sync 는 P44 vault git pull 패턴 그대로 자연 동작.
- **DB `sessions.is_archived` 는 캐시** — ingest 시 frontmatter → DB 단방향 동기화. archive/restore 호출은 두 곳을 트랜잭션으로 묶음.
- **FTS5 + sessions JOIN + partial index** — 검증된 SQL 패턴 (P11). denormalize / post-filter 회피.
- **archive 만 vault sync 범위** — 기존 favorite / tags / notes 의 vault 동기화는 별도 plan (DB only 동작 유지).

## Expected Outcome

- `archive_session(id)` / `restore_session(id)` 호출 시 DB row 와 vault frontmatter 양쪽 정합성 유지 (한쪽 실패 시 rollback).
- 윈도우에서 archive → 맥에서 `secall sync` 시 vault git pull → ingest 재실행 → DB 의 `is_archived` 자동 반영.
- 기본 `recall` / `list` / BM25 / hybrid 검색에서 archived 세션 제외, `include_archived` 옵션으로 토글 가능.
- 기존 회귀 0 — `is_archived = 0` 데이터에 대한 모든 기존 동작 변화 없음.

## Subtask Summary

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 01 | DB migration (schema v10) — `is_archived` / `archived_at` + partial index | A | — |
| 02 | Vault frontmatter writer 확장 — `archived` / `archived_at` 직렬화 + in-place updater | A | — |
| 03 | Ingest frontmatter parser 확장 — `archived` / `archived_at` 읽어 DB UPSERT 시 반영 | A | — |
| 04 | Store `archive_session` / `restore_session` — DB + vault frontmatter 트랜잭션 | B | 01, 02 |
| 05 | 기본 list / search / recall / hybrid 에 `is_archived = 0` filter 적용 | B | 01 |
| 06 | 회귀 테스트 — round-trip + filter 동작 + cross-host re-ingest sync | C | 03, 04, 05 |

## Constraints

- Vault frontmatter 의 다른 필드 (favorite/tags/notes) 동작 변화 없음 — 기존 DB only 유지.
- 기존 `sessions` row 는 모두 `is_archived = 0` 기본값 — backwards compat.
- Schema v10 migration 은 idempotent (`column_exists` 가드 + PRAGMA user_version 비교).
- `archive_session` 의 vault write 실패 시 DB rollback — DB 와 vault 의 정합성 깨지지 않도록 트랜잭션 / 에러 경로 명확화.

## Non-goals

- REST `archive`/`restore` endpoint (P46).
- Web UI badge / action / dialog (P46).
- Cleanup candidate classifier (P47).
- CLI `secall session archive/restore/delete/prune` subcommand (P46/P47).
- Hard delete + `delete_session_full_with_options` (P46).
- wiki update / graph snapshot 의 archive 필터 (P49).
- `favorite` / `tags` / `notes` 의 vault frontmatter 동기화 (P50+).
- archived 세션 source 의 wiki UI 시각화 (P49).
- multi-host bulk archive job 시스템 연동.

## Plan version

v1.0 (2026-05-12) — 최초 작성.
