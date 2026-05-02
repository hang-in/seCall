# Review Report: P34 — secall-web 뷰어 본격 강화 (Phase 2: 탐색 깊이 + UX) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-02 21:06
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/src/mcp/rest.rs:283 — `SessionListQuery.tags`가 `Option<String>`이라 `?tags=rust&tags=search` 형태를 받을 수 없습니다. Task 03 문서의 계약은 콤마 구분과 반복 파라미터 둘 다 지원인데, 현재 구현은 단일 문자열만 split하므로 반복 형태 API가 깨집니다.
2. web/src/components/SessionList.tsx:52 — `useListHotkeys()`가 어디에서도 호출되지 않습니다. 그래서 Task 04의 핵심 요구사항인 `j/k`, `Enter`, `[`, `]` 리스트/세션 이동 단축키는 도움말에만 있고 런타임에서는 등록되지 않습니다.
3. web/src/components/RelatedSessions.tsx:35 — 관련 세션 클릭 시 `navigate(\`/sessions/${it.id}\`)`로 raw ID를 라우트에 넣습니다. 세션 ID에 `/`, `?`, `#` 같은 예약 문자가 있으면 상세 페이지 이동이 깨집니다.
4. web/src/components/GraphOverlay.tsx:51 — 그래프에서 세션 노드를 클릭할 때도 `navigate(\`/sessions/${nodeId}\`)`를 사용합니다. 동일하게 raw ID 경로 인코딩 누락으로 일부 세션 상세 이동이 실패할 수 있습니다.

## Recommendations

1. `tags`는 `Vec<String>`로 받거나, repeated query를 안전하게 수용하는 파서로 바꾼 뒤 콤마 구분 입력과 병합 처리하는 편이 맞습니다.
2. 세션 이동은 공통 헬퍼로 묶어서 `encodeURIComponent(id)`를 강제하면 같은 회귀를 막을 수 있습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | DB 스키마 v7 | ✅ done |
| 2 | 시맨틱 검색 모드 활성 | ✅ done |
| 3 | 검색어 하이라이트 | ✅ done |
| 4 | 다중 태그 필터 + 날짜 quick range | ✅ done |
| 5 | 키보드 단축키 | ✅ done |
| 6 | 관련 세션 패널 | ✅ done |
| 7 | 그래프 시각화 강화 | ✅ done |
| 8 | 세션 메타 mini-chart | ✅ done |
| 9 | 세션 노트 편집 UI | ✅ done |
| 10 | README + CI | ✅ done |

