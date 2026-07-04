# seCall 웹 세션 로딩 검토 — 지연로딩 / RAM

> 질문: "인제스트한 세션 로그를 웹에서 레이지로딩처럼 천천히 다 로딩해 둘 수 있나? 램 압박인가?"
> 검토 대상: `web/src` (React + Vite + @tanstack/react-query + zustand). 관찰일 2026-07-04.

## 요약 (결론)

- **데이터 로딩은 이미 lazy임 — "preload-all"이 아님.** 리스트는 페이지네이션 무한스크롤, 본문은 열 때만 fetch. → **데이터 측 RAM 압박은 이미 없음.**
- **진짜 문제는 "DOM 누적"** — 무한스크롤이 **가상화(windowing) 없이** 로드된 아이템 전량을 DOM에 쌓음. 수천 개 스크롤 시 DOM 노드 수천 개 → 메모리·렉 증가.
- **긴 세션 본문**도 transcript 전체를 한 번에 react-markdown 렌더 → 큰 DOM + 무거운 파싱.
- → **preload-all 하지 말고, "가상화 + 이미 있는 온디맨드 fetch"** 로 가면 램 상수. 아래 권장.

---

## 현 상태 (코드 근거)

### 리스트 — `components/SessionList.tsx`, `hooks/useSessions.ts`
- `useInfiniteSessions` = react-query `useInfiniteQuery`, 백엔드 `/api/sessions?page=N&page_size=50` (`{items,total,page,page_size}`). `getNextPageParam`로 종료 판정. ✅ **온디맨드**.
- `useInfiniteScroll` = IntersectionObserver sentinel, `rootMargin:"200px"`로 끝 200px 전 prefetch, `hasMore=false`면 미설정. ✅ 깔끔(폴링 아님).
- **문제**: `allItems = data.pages.flatMap(p => p.items)` → 로드된 **모든** 아이템을 `.map`으로 렌더. **가상화 라이브러리 없음**(deps에 react-window/virtua/tanstack-virtual 부재). → 스크롤할수록 `SessionListItem` DOM 노드 **무한 누적**.

### 본문 — `routes/SessionDetailRoute.tsx`, `components/MarkdownView.tsx`
- `useSession(id, true)` → 본문(`content`) **열 때만** fetch. ✅
- `MarkdownView`가 `content` **전체 문자열**을 `ReactMarkdown`(remark-gfm/wiki-link/callouts + rehype-raw/highlight/sanitize)로 **한 번에** 렌더. → 아주 긴 transcript면 큰 DOM + rehype-highlight 파싱 비용 큼. **청킹/가상화 없음**.

### 캐시
- react-query 기본 gcTime(5분) 동안 로드된 페이지 + 연 세션 본문을 메모리 유지. 큰 본문 여러 개 연달아 열면 잠깐 누적. (튜닝 여지)

---

## RAM 감각

| | 현재 | 위험 |
|---|---|---|
| 리스트 데이터 | 페이지(50)씩 fetch | 낮음(로드분만) |
| **리스트 DOM** | **전량 `.map`** | **높음 — 스크롤 누적** |
| 본문 데이터 | 온디맨드 | 낮음 |
| **본문 DOM/파싱** | **transcript 통째 렌더** | 세션이 크면 높음 |

핵심: "세션 로그가 verbose(MB급) × 수백~수천" 이라 **전량 상주는 GB급 → 불가**. 근데 이미 데이터는 상주 안 함. **남은 건 DOM 상수화(가상화).**

---

## 권장 (우선순위)

### P0 — 리스트 가상화 (windowing)
- **`@tanstack/react-virtual` 도입**(이미 `@tanstack/react-query` 쓰므로 결이 맞음). react-window/virtua도 가능.
- `SessionList`에서 `allItems`를 virtualizer로 감싸 **보이는 ~30행만 DOM**에. 무한스크롤과 조합은 TanStack 표준 레시피(virtualizer의 마지막 인덱스 근접 시 `fetchNextPage`).
- 효과: 세션 1만 개든 10만 개든 **DOM 노드 상수** → 스크롤 램·렉 해소. **"천천히 다 볼 수 있게"의 정답.**
- 주의: `SessionListItem` 높이 가변이면 dynamic measurement(`measureElement`) 사용.

### P1 — 긴 세션 본문 지연 렌더
세션이 크면(수천 turn) 통째 렌더가 부담. 택1/조합:
- **(a) turn 단위 청크 + 무한스크롤**: 세션은 `turn_count`가 있으니 turn 블록으로 나눠 **첫 N turn 렌더 → 스크롤 시 IntersectionObserver로 이어붙임**(리스트와 같은 패턴). 이게 사용자가 말한 "레이지로딩"의 본문 버전. (백엔드가 turn-range 본문 서빙하면 데이터도 청크.)
- **(b) `content-visibility:auto`** (거의 공짜): turn/블록 컨테이너에 CSS 적용 → 브라우저가 오프스크린 블록의 레이아웃·페인트 스킵. DOM 노드 수는 그대로지만 렌더 비용 급감. **먼저 이거부터** 넣으면 저비용 개선.
- (c) react-markdown 결과 `useMemo`(이미 함) + rehype-highlight는 코드블록 많으면 무거우니, 초대형 문서에선 지연/워커 파싱 고려.

### P2 — 캐시 바운딩
- 본문 detail 쿼리의 `gcTime`을 낮추거나(예 60s), 매우 큰 본문은 캐시에서 빨리 비우게 → 큰 세션 여러 개 열 때 메모리 상한.

---

## 한 줄 답
- **"다 로딩해두기"는 하지 마(램 압박 맞음).** 이미 데이터는 lazy니, **리스트에 가상화(P0) + 긴 본문에 `content-visibility`/turn 청크(P1)** 만 얹으면 "전부 있는 것처럼 부드럽게" + 램 상수. 백엔드(FTS/REST/페이지네이션)는 이미 이걸 받쳐줌.
