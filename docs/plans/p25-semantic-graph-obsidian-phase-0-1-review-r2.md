# Review Report: P25 — Semantic Graph 활용 + Obsidian 플러그인 (Phase 0-1) — Round 2

> Verdict: fail
> Reviewer: 
> Date: 2026-04-14 18:19
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. obsidian-secall/src/session-view.ts:55 — `SessionView`가 세션 메타 줄에서 `data.start_time`을 사용하지만, `crates/secall-core/src/mcp/server.rs:159-195`의 `do_get()`가 반환하는 세션 JSON에는 `start_time`이 없고 `date`만 있습니다. 이 상태로는 fallback 상세 뷰 상단 메타가 `… · undefined`로 표시됩니다.

## Recommendations

1. `obsidian-secall/src/search-view.ts:64-67`의 vault root 계산은 `adapter.basePath`가 비어 있을 때 `startsWith(\"\")`가 항상 참이어서 경로 첫 글자를 잘못 잘라낼 수 있습니다. `vaultRoot`가 truthy일 때만 prefix 제거를 수행하도록 방어하는 편이 안전합니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | REST API 서버 (secall serve) | ✅ done |
| 2 | Obsidian 플러그인 scaffold + recall | ✅ done |
| 3 | 세션 조회 + 상태바 | ✅ done |

