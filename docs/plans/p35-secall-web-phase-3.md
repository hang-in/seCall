---
type: plan
status: draft
updated_at: 2026-05-02
slug: p35-secall-web-phase-3
version: 1
---

# P35 — secall-web Phase 3 (성능 + 정확도)

## Description

P34에서 의도적으로 분리한 3가지 항목 — **번들 크기 축소(code-split)**, **세션 리스트 무한 스크롤**, **`/api/tags` 정확한 전체 태그 목록** — 을 한 Phase로 묶어 처리. 기능 추가가 아닌 "현재 한계 해소" 성격.

## 현재 한계

- Vite 단일 chunk 978 kB (warning) — 초기 로드 비용 큼. 변경 시 전체 캐시 무효화.
- `web/src/components/SessionList.tsx`는 page_size=100 단발 호출 — 100개 초과 세션 미표시.
- `web/src/lib/allTags.ts`의 `useAllTags`는 sessions 100건의 `tags` 합집합 휴리스틱 — 100건 너머 태그 누락 → TagEditor/SessionFilters 자동완성 부정확.

## Expected Outcome

- 초기 진입 시 다운로드 ≤ 350 kB (gzip), `vendor-react` / `vendor-query` / `vendor-radix` / `vendor-viz` / per-route chunk로 분리.
- SessionList: 스크롤 끝 도달 시 자동으로 다음 페이지 로드, 모든 세션 접근 가능.
- `useAllTags`: 백엔드 `/api/tags` 호출 (sessions 테이블 전체 스캔 + json_each) — 모든 태그 + 사용 빈도 정확.

## Subtasks

| # | Title | Parallel group | Depends on |
|---|---|---|---|
| 00 | 백엔드 `/api/tags` 엔드포인트 | A | — |
| 01 | web `useAllTags`를 `/api/tags`로 전환 | B | 00 |
| 02 | SessionList 무한 스크롤 | A | — |
| 03 | Code-split (라우트 + vendor) | A | — |
| 04 | README + CI 업데이트 | C | 00, 01, 02, 03 |

병렬 실행 전략:
- Phase A — Task 00 + 02 + 03 동시 dispatch (서로 다른 파일군)
- Phase B — Task 01 (Task 00의 endpoint 필요)
- Phase C — Task 04 (모든 task 완료 후 정확한 정보 반영)

## Constraints

- **수정 금지**: P32~34 완료 코드의 동작 변경. 본 phase는 추가 또는 동치 교체만.
- **무한 스크롤은 keyword 모드 한정**. semantic 모드(`do_recall`)는 서버가 페이지네이션 제공 안 함 → 단발 유지.
- `/api/tags`는 sessions 테이블 전체 스캔. 현재 규모(수만 건)에서 OK. 향후 더 커지면 별도 tags 테이블은 Phase 4+.

## Non-goals

- 가상 스크롤 (react-window 등): 무한 스크롤로 충분, 가상화는 별도 phase.
- cursor 기반 페이지네이션: 현재 offset/limit 방식 유지.
- semantic 모드 페이지네이션: do_recall 변경 필요 → 별도 phase.
- 태그 관리 UI (rename / merge / bulk delete): 별도 phase.
- `/api/tags` 검색 fuzzy 매칭: 클라 측 substring 필터로 충분.

## Success criteria

- `pnpm build` chunk size warning 사라짐 (모든 chunk ≤ 500 kB)
- `cargo test --all` 통과 + 신규 `/api/tags` 통합 테스트 통과
- `useAllTags` 결과가 sessions 100건 한계와 무관 (DB의 모든 tags 노출)
- SessionList 무한 스크롤이 IntersectionObserver로 작동, 마지막 페이지 도달 시 sentinel 사라짐
