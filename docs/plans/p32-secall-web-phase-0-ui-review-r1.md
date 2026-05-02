# Review Report: P32 — secall-web Phase 0 (읽기 전용 웹 UI + 태그/즐겨찾기) — Round 1

> Verdict: fail
> Reviewer: 
> Date: 2026-05-02 17:19
> Plan Revision: 0

---

## Verdict

**fail**

## Findings

1. crates/secall-core/tests/rest_listing.rs — Task 02의 Changed files에 필수 신규 통합 테스트로 명시됐지만 저장소에 파일이 없습니다. required file 미구현이라 task 계약을 충족하지 못합니다.
2. web/src/routes/SessionDetailRoute.tsx:15, web/src/components/SessionHeader.tsx:27 — 세션 상세가 tags/is_favorite를 서버 응답이 아니라 sessions/allTags 캐시에서만 보강하고, 캐시 미스 시 빈 태그와 `false`로 폴백합니다. `/daily`, `/wiki`, 그래프 오버레이에서 직접 `/sessions/:id`로 진입하면 기존 태그/즐겨찾기 상태가 누락되어 잘못 표시되고, 이후 편집 시 저장된 메타데이터를 덮어쓸 수 있습니다.

## Recommendations

1. `/api/get` 응답에 `tags`, `is_favorite`, `turn_count`, `start_time`를 포함시키거나, 상세 화면이 별도 메타 조회를 수행하도록 바꾸는 편이 안전합니다.
2. web/src/lib/allTags.ts:16 의 첫 100개 세션 제한은 자동완성 정확도를 떨어뜨리므로, 전용 `/api/tags` 엔드포인트나 전체 페이지 순회로 교체하는 편이 좋습니다.

## Subtask Verification

| # | Subtask | Status |
|---|---------|--------|
| 1 | 워크스페이스 + 빌드 파이프라인 | ✅ done |
| 2 | rust-embed + Vite reverse proxy 통합 | ✅ done |
| 3 | 신규 REST 엔드포인트 추가 | ✅ done |
| 4 | DB 스키마 v5 마이그레이션 | ✅ done |
| 5 | React 프론트 핵심 셋업 | ✅ done |
| 6 | 2-pane 레이아웃 + 검색/세션 뷰 구현 | ✅ done |
| 7 | 일일 일기 + 위키 + 태그/즐겨찾기 UI | ✅ done |
| 8 | 그래프 폴딩 오버레이 | ✅ done |
| 9 | README + CI 업데이트 | ✅ done |

