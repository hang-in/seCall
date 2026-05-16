---
type: reference
status: in_progress
updated_at: 2026-05-16
---

# secall-web 백로그 / 알려진 이슈

> secall-web (`secall serve` 가 serve 하는 React + Vite SPA) 관련 미해결 / 추적 항목.
> GitHub Issues 트래커가 SSOT 가 아닌 동안 (사용자 운영 부담 고려) 이 markdown 이 단일 진실 원천.
> 발견 즉시 추가하고, 처리 완료 시 "완료" 섹션으로 이동 (또는 PR 머지 후 삭제).

---

## 분류

각 항목은 다음 4분류 중 하나:

- **🔥 hot**: 사용자 보고/관찰 기반의 명백한 결함 — 다음 작업 단위에 포함
- **🟡 debt**: 구조적/장기적 부채 — 시간 날 때 처리
- **🔵 watch**: 관찰 항목 — 현재는 문제 아니지만 회귀 가능성
- **✅ done**: 처리 완료 (다음 정리 사이클 때 삭제 후보)

---

## hot

### Graph snapshot 대용량 응답 → "멈춤 화면"
- **위치**: `crates/secall-core/src/mcp/server.rs:do_graph_snapshot` + `web/src/lib/api.ts:183` (`graphSnapshot(sessionLimit=80)`) + `web/src/routes/GraphRoute.tsx` (ObsidianGraph)
- **현상**: server 응답에 **edge 개수 제한이 없음**. `session_limit=80` 만 있고 그 안의 모든 관계 (`by_agent` / `belongs_to` / `discusses_topic` / `same_project` / `same_day`) 가 직렬화됨. 2026-05-16 시점 응답 약 **1246 edges / ~600KB**. web 측은 D3 force-simulation + SVG 로 모든 노드/엣지를 DOM 렌더링 (virtualization 없음). 사용자 보고 "그래프 거의 멈춤화면" 과 직접 연결됨.
- **권장 fix**:
  - server: `/api/graph/snapshot` 에 `edge_limit` 파라미터 (또는 relation 별 cap) 추가
  - client: edge 수 임계값 (예: 300+) 이면 자동 filter / cluster / canvas-mode 전환

### LLM config 모델 자유 입력 (드롭다운 부재)
- **위치**: `web/src/routes/SettingsRoute.tsx` 7곳 — `review_model:275` / `ollama_model:366` / `anthropic_model:371` / `cloud_model:376` / `log.model:122` / `log.cloud_model:127` / `embedding.openai_model:137`
- **현상**: 모델 필드가 모두 `<Input type="text">` 자유 입력. 검증은 `validateModelName()` (정규식 `/^[a-zA-Z0-9._:-]+$/`) 만. backend 별 (ollama/ollama_cloud/lmstudio/anthropic/openai/gemini) 추천 모델 hardcode 미존재. 반면 `semantic_backend` 는 정상적으로 `<select>` 드롭다운 사용. 타이포 위험 + 사용자가 가용 모델 모를 때 시행착오.
- **권장 fix**:
  - 옵션 a: `web/src/lib/api.ts` 에 backend 별 모델 목록 상수 + `<datalist>` 패턴 (드롭다운 + 자유 입력 fallback)
  - 옵션 b: server 측 `/api/config/models?backend=...` endpoint 신설 (SSOT)
  - graph / wiki / log / embedding 4 섹션 모두 동일 패턴 적용

## debt

### dist 빌드 누락 회귀
- **위치**: `web/dist/` + `crates/secall-core/src/web/embed.rs` (`rust_embed`)
- **현상**: `web/dist/` 는 `rust_embed::RustEmbed#[folder = "../../web/dist/"]` 로 secall binary 에 build 시점 embedded. cargo 는 `web/dist/` 디렉토리 변경을 **자동 감지 안 함** — `pnpm build` 를 선행하지 않고 `cargo install --path crates/secall --force` 만 돌리면 옛 dist 스냅샷이 그대로 embed.
- **재현**: 2026-05-15 시점 `web/dist/index.html` 빌드 시각이 `2026-05-09 06:36`. P54 web redesign 머지 후 `cargo install` 만 돌린 환경에선 옛 번들 (v0.4.2 hardcode 포함) 이 serve 됨. 2026-05-16 P62 작업 중 발견.
- **현재 회피책**: release / install 절차에 `cd web && pnpm build && cd .. && cargo install --path crates/secall --force` 순서 명시. 단순 `cargo install` 만 돌리면 미반영.
- **후속 액션 후보**:
  - `crates/secall-core/build.rs` 에 `cargo:rerun-if-changed=../../web/dist/index.html` 추가해 dist 변경 시 cargo rebuild 트리거
  - 또는 `cargo xtask build` 류 wrapper 가 pnpm build + cargo build 묶어 실행
  - 또는 release 절차에 "pnpm build 후 cargo install" 체크리스트 박제

### Markdown 폴딩/언폴딩 미구현
- **위치**: `web/src/components/MarkdownView.tsx`
- **현상**: `react-markdown` + `remarkGfm` 만 로드. `<details>`/`<summary>` 컴포넌트 override 없음. heading 클릭으로 섹션 접기 없음. 비교: `NoteEditor.tsx:57` / `SessionHeader.tsx:61` 은 `<details>` 직접 사용해 동작. 사용자 보고 "폴딩 제대로 안 됨" 과 일치.
- **권장 fix**: `components` 객체에 `h1/h2/h3` override (click handler + collapsed state) 또는 `remark-collapse` 류 플러그인. 동시에 `details` override 로 click 동작 명시화.

### `jfetch<unknown>` 타입 미해결 6 endpoints
- **위치**: `web/src/lib/api.ts` — `daily:162` / `graphSearch:176` / `wikiSearch:179` / `graphSnapshot:183` / `wikiList:188` / `cancelJob:238`
- **현상**: 응답 타입이 모두 `unknown`. 사용처에서 assertion / `as any` 처리. P62 에서 `StatusResponse` 만 정의됐고 나머지는 방치.
- **권장 fix**: `web/src/lib/types.ts` 에 `DailyResponse` / `WikiSearchResponse` / `WikiListResponse` / `GraphSearchResponse` / `GraphSnapshot` / `JobCancelResponse` 추가 후 `jfetch<...>` 갱신.

### SessionsRoute 좌측 리스트 에러 표시 없음
- **위치**: `web/src/routes/SessionsRoute.tsx` (없음) vs `SessionList.tsx:110-119` / `SessionDetailRoute.tsx:18-23` / `DailyRoute.tsx:91` / `WikiRoute.tsx:78,128,176` (있음)
- **현상**: keyword/semantic search 실패가 SessionsRoute 최상위에 표시되지 않음. SessionList 내부에서만 isError 체크 — 빈 결과와 구분 어려움. 다른 라우트는 모두 error banner 표시.
- **권장 fix**: SessionList 에서 error state 를 prop 으로 expose 또는 SessionsRoute 가 `useInfiniteSessions` / `useSemanticRecall` 의 error 를 직접 catch 해 상단 alert.

### `/api/status` 가 stats 쿼리와 묶임
- **위치**: `crates/secall-core/src/mcp/server.rs:do_status`
- **현상**: version 만 확인하려는 client 도 DB lock + stats 쿼리 (sessions/turns/vectors count) 비용을 지불.
- **후속 액션 후보**: client 가 version 만 필요한 경우용 cheap `/api/version` 신설, 또는 web 측 react-query cache (long stale time).
- **우선순위**: 현재 호출 빈도 낮음 (TopNav mount 시 1회). 회귀 모니터링 후 결정.

## watch

### react-query `staleTime` 보수적
- **위치**: `web/src/hooks/useSessions.ts:20` (projects/agents `staleTime: 60_000`) + `web/src/hooks/useWiki.ts:40` (`60_000`)
- **현상**: projects / agents / wiki list 가 자주 변하지 않는데 1분마다 refetch. 네트워크 비용 / 불필요한 backend 호출.
- **권장 fix**: `staleTime: 5 * 60_000` (5분) 으로 상향 + mutation 시점에 `queryClient.invalidateQueries` 명시 호출.

### Mobile / tablet 미지원
- **위치**: `web/src/routes/SessionDetailRoute.tsx:42` `grid grid-cols-1 lg:grid-cols-[...]` (md breakpoint 없음)
- **현상**: tailwind breakpoint 가 `lg` (1024px) 만. 태블릿 (768~1023px) 구간은 1열 collapse 또는 미정의 layout.
- **권장 fix**: `md:` breakpoint 추가 (`md:grid-cols-[minmax(0,var(--read-w))_300px]`) 또는 mobile-first 패턴 점진 도입.

### Markdown link 컴포넌트 미커스텀
- **위치**: `web/src/components/MarkdownView.tsx`
- **현상**: `[text](url)` 이 그냥 `<a href=...>` 로. vault 내부 path / `[[wikilink]]` obsidian syntax 미지원. 외부 URL 만 동작.
- **권장 fix**: `components: { a: CustomLink }` 추가 (vault path detection + react-router navigation) + 필요 시 `remark-wiki-link` 플러그인.

### server-side version 과 web build 시점 version 불일치
- **시나리오**: server 만 `cargo install --force` 로 새 버전 깔고 `secall serve` 띄우면 — server 의 `/api/status` 는 새 버전, web bundle (dist) 은 옛 빌드 시점에 박힌 자산 (CSS/JS layout). 표시되는 version 은 server 가 SSOT 이므로 일치하지만, 실제 화면 동작은 옛 dist 기준.
- **현재 상태**: `Build web/dist` CI job 이 PR 마다 빌드해 dist 가 src 와 sync 유지. 단 release tag 자동화는 없음 — 수동 release 시 dist 누락 가능.

---

## 처리 절차

1. 새 항목 발견 → 분류 (hot / debt / watch) 후 본 문서 해당 섹션에 추가
2. 항목 처리 시 별도 PR + 커밋 메시지에 본 문서의 항목 명시
3. PR 머지 후 본 문서에서 항목 제거 (또는 done 섹션으로 잠시 이동)
4. 분기 (월 1회 등) 마다 본 문서를 가볍게 청소
