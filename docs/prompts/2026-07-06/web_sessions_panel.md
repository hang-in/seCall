---
type: prompt
status: draft
updated_at: 2026-07-06
---

# 세션 좌측 패널 고도화 — 정렬 + 필터 + 달력

secall-web 좌측 세션 리스트 패널 강화. **백엔드(Rust) + 프론트(React) 풀스택**. Phase 로 나눠 진행.

## 배경 (반드시 Read 로 현재 구현 파악)
- 프론트 리스트: `web/src/components/SessionList.tsx`(무한스크롤/가상화/시맨틱), `SessionListItem.tsx`, 필터 UI, `web/src/routes/SessionsRoute.tsx`.
- 데이터 훅: `web/src/hooks/useSessions.ts` (`useInfiniteSessions`/`useSessionsList`/`useSemanticRecall`), `web/src/lib/api.ts` (`listSessions`), `web/src/lib/types.ts` (`SessionsListParams`, `SessionFilterState`).
- 백엔드: `crates/secall-core/src/mcp/rest.rs` (`/api/sessions` = `api_list_sessions`), `crates/secall-core/src/store/session_repo.rs` (`SessionListFilter`, list 쿼리).
- 현재 정렬은 `start_time DESC` 고정. 필터는 project/agent/date range/tags/favorite.

## Phase 1 — 정렬 옵션
- 정렬 기준: **날짜(start_time) / turns(turn_count) / 프로젝트(project) / 에이전트(agent)**, 각 asc/desc.
- 백엔드: `SessionsListParams`/`SessionListFilter` 에 `sort`(enum: date|turns|project|agent) + `order`(asc|desc) 추가. `session_repo` list 쿼리의 `ORDER BY` 를 파라미터화(화이트리스트로 SQL 인젝션 방지 — 컬럼명을 match 로 고정). rest `api_list_sessions` 가 쿼리스트링에서 받아 전달. **기본값은 date desc(현행 동작 보존)**.
- 프론트: `api.listSessions`/`useInfiniteSessions` 에 sort/order 전달. SessionList 상단에 **정렬 드롭다운(shadcn Select)** 추가. 변경 시 query key 에 포함돼 자동 refetch.
- keyword 경로에만 적용(semantic recall 은 score 정렬이라 제외 — UI 에서 semantic 모드일 땐 정렬 컨트롤 비활성/숨김).

## Phase 2 — 필터 보강 (여유 되면)
- 기존 필터(project/agent/date range/tags/favorite) 유지. 세션 타입(interactive/automated) 토글 정도만 얇게 추가 검토(automated 는 기본 제외 유지). 과하게 늘리지 말 것.

## Phase 3 — 달력 (날짜별 세션 수 + 클릭 필터)
- **백엔드 신규 endpoint**: 날짜별 세션 count. 예 `GET /api/sessions/calendar?from=YYYY-MM-DD&to=YYYY-MM-DD` → `[{date, count}]`. `session_repo` 에 `SELECT DATE(start_time, tz_offset) AS d, COUNT(*) ... GROUP BY d` (tz offset 은 #131 데일리와 동일하게 브라우저 offset 전달받아 로컬 날짜 기준). automated/노이즈 제외는 데일리(do_daily)와 동일 기준 권장.
- **프론트 달력 컴포넌트**: 월 단위 미니 캘린더. 각 날짜 셀에 **세션 수 배지**(0이면 흐리게/생략). 날짜 클릭 → 해당 날짜로 date 필터 적용(리스트 필터링). 월 이동(이전/다음). 위치는 좌측 패널 상단 접이식 또는 필터 영역 내.
- shadcn 에 calendar(react-day-picker) 가 있으면 활용, 없으면 간단 그리드로 자체 구현. index.css 토큰만 사용.

## Constraints
- **기본 동작 보존**: 정렬/필터 미지정 시 현행(date desc, 기존 필터)과 100% 동일.
- 도메인 로직(무한스크롤/가상화/삭제 낙관/시맨틱/단축키) 불변. 데이터 훅 시그니처는 확장만(기존 호출 깨지지 않게 optional).
- 백엔드: `cargo check`/`cargo test`(lib) 통과, **integration 테스트(tests/)의 기존 assert 깨지지 않게** (schema/route 변경 시 확인). ORDER BY 는 화이트리스트 컬럼만.
- 프론트: tsc/vite build/vitest 통과. 다크/라이트 둘 다. 반응형(모바일에서 정렬/달력이 깨지지 않게).
- 커밋 금지. Phase 별로 무엇을 바꿨는지 파일:라인 요약 반환.

## 권장 순서
Phase 1(정렬) → Phase 3(달력, 백엔드 count API 포함) → Phase 2(필터 보강). Phase 1 만이라도 완결되면 가치 있음.
