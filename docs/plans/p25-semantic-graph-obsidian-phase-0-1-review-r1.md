# Review Report: P25 — Semantic Graph 활용 + Obsidian 플러그인 (Phase 0-1) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-04-14 18:13
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. obsidian-secall/src/search-view.ts:55 — `openSession()`과 결과 렌더링이 `vault_path`, `project`, `agent`, `date`, `summary`를 top-level 필드로 가정하지만, backend `do_recall()`은 `SearchResult`를 그대로 반환해서 메타데이터가 `metadata.{vault_path, project, agent, date}` 아래에 옵니다. 이 상태로는 vault 파일 열기 분기가 동작하지 않고, 검색 결과 메타도 비어 보입니다.
2. crates/secall-core/src/mcp/server.rs:161 — `do_get(..., full=true)`는 `meta.vault_path`가 있을 때만 `content`를 채웁니다. 그래서 Task 03의 “vault_path 없는 세션 클릭 → SessionView에 메타+본문 표시” 경로에서 `obsidian-secall/src/session-view.ts:59` 이하의 본문 렌더링이 실행되지 않아, fallback 상세 뷰가 본문 없이 비어 있게 됩니다.

## Recommendations

1. recall 응답을 플러그인 기대 형태로 평탄화하거나, `search-view.ts`에서 `r.metadata`를 읽도록 타입과 렌더링을 맞추세요.
2. `GET /api/get`의 `full=true`는 vault markdown이 없을 때 DB turn들을 합쳐 `content`를 만들어 주거나, 별도 상세 조회 payload를 정의해 SessionView가 항상 본문을 렌더링할 수 있게 하세요.
3. result 문서의 Task 03 verification 블록이 중간에서 잘려 있어 추적성이 떨어집니다. 재작업 시에는 전체 검증 결과가 artifact에 남는지 확인하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | REST API 서버 (secall serve) | ✅ done |
| 2 | Obsidian 플러그인 scaffold + recall | ✅ done |
| 3 | 세션 조회 + 상태바 | ✅ done |

