---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 03
parallel_group: A
depends_on: []
---

# Task 03 — 다중 태그 필터 + 날짜 quick range

## Changed files

수정:
- `crates/secall-core/src/store/session_repo.rs` — `SessionListFilter.tag` (`Option<String>`) → `tags` (`Vec<String>`)로 확장 (단일 태그 호환 유지 위해 신규 필드 + deprecated 표시)
- `crates/secall-core/src/mcp/rest.rs` — `SessionListQuery.tag` 단일 + `tags` 다중 (콤마 구분 또는 반복) 둘 다 받음
- `web/src/components/SessionFilters.tsx` — 단일 select → 다중 chips + 날짜 quick range 버튼 4개
- `web/src/lib/types.ts` — `SessionFilterState.tags: string[]` 추가, `SessionsListParams`도 갱신
- `web/src/lib/api.ts` — `listSessions`가 `tags`를 콤마 구분 또는 반복 query string으로 전달
- `web/src/hooks/useSessions.ts` — 시그니처 호환

신규: 없음

## Change description

### 1. 백엔드 (`SessionListFilter` 확장)

```rust
pub struct SessionListFilter {
    pub project: Option<String>,
    pub agent: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub tag: Option<String>,        // 단일 태그 (P32 호환)
    pub tags: Vec<String>,          // 다중 태그 AND 매칭 (P34 신규)
    pub favorite: Option<bool>,
    pub q: Option<String>,
    pub page: usize,
    pub page_size: usize,
}
```

`list_sessions_filtered`의 SQL 조립에 추가:
```rust
// 단일 태그 (P32 호환)
if let Some(t) = &f.tag {
    conditions.push("tags LIKE ?".to_string());
    params.push(Box::new(format!("%\"{}\"%", t.replace('"', "\"\""))));
}
// 다중 태그 AND
for t in &f.tags {
    conditions.push("tags LIKE ?".to_string());
    params.push(Box::new(format!("%\"{}\"%", t.replace('"', "\"\""))));
}
```

각 태그가 별도 LIKE → AND 매칭. 정규화는 클라이언트 입력 시 보장. 빈 벡터는 영향 없음.

### 2. REST `SessionListQuery`

```rust
#[derive(Deserialize, Default)]
struct SessionListQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    project: Option<String>,
    agent: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    tag: Option<String>,
    tags: Option<String>,        // 콤마 구분 ("rust,search")
    favorite: Option<bool>,
    q: Option<String>,
}

impl From<SessionListQuery> for SessionListFilter {
    fn from(q: SessionListQuery) -> Self {
        let tags: Vec<String> = q
            .tags
            .as_deref()
            .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        SessionListFilter {
            project: q.project,
            agent: q.agent,
            date_from: q.date_from,
            date_to: q.date_to,
            tag: q.tag,
            tags,
            favorite: q.favorite,
            q: q.q,
            page: q.page.unwrap_or(1),
            page_size: q.page_size.unwrap_or(30),
        }
    }
}
```

### 3. Web `SessionFilterState`

```ts
export interface SessionFilterState {
  project?: string;
  agent?: string;
  date_from?: string;
  date_to?: string;
  tags?: string[];          // P34 신규 — 다중 태그 AND
  tag?: string;             // P32 호환 — 단일
  favorite?: boolean;
}
```

### 4. SessionFilters UI

기존 단일 select → 칩 입력:
- 등록된 태그 자동완성 (P32 `useAllTags`)
- 칩 X로 제거
- 추가 시 정규화 (소문자 + 공백→`-`)

날짜 quick range 버튼 4개: 오늘 · 이번 주 · 이번 달 · 직접 선택. 클릭 시 date_from/date_to 자동 설정.

```tsx
const today = format(new Date(), "yyyy-MM-dd");
const startOfThisWeek = format(startOfWeek(new Date(), { weekStartsOn: 1 }), "yyyy-MM-dd");
const startOfThisMonth = format(startOfMonth(new Date()), "yyyy-MM-dd");

<Button onClick={() => setFilters({ ...filters, date_from: today, date_to: today })}>오늘</Button>
<Button onClick={() => setFilters({ ...filters, date_from: startOfThisWeek, date_to: today })}>이번 주</Button>
<Button onClick={() => setFilters({ ...filters, date_from: startOfThisMonth, date_to: today })}>이번 달</Button>
<Button variant="outline" onClick={() => setFilters({ ...filters, date_from: undefined, date_to: undefined })}>전체</Button>
```

`date-fns`의 `startOfWeek`, `startOfMonth` import (이미 P32에서 추가됨).

### 5. API 호출

`api.listSessions`에 `tags` 콤마 구분으로 전달:
```ts
listSessions: (params) => {
  const qs = new URLSearchParams();
  Object.entries(params).forEach(([k, v]) => {
    if (v === undefined) return;
    if (Array.isArray(v)) {
      if (v.length > 0) qs.set(k, v.join(","));
    } else {
      qs.set(k, String(v));
    }
  });
  return jfetch<SessionListPage>(`/api/sessions?${qs}`);
},
```

### 6. 단위 테스트

- `db.rs` tests: `test_list_sessions_multi_tag_and` — 두 태그 모두 가진 세션만 매칭
- `tests/rest_listing.rs`: 다중 태그 시나리오 추가

## Dependencies

- 외부 crate: 없음 (date-fns 이미 있음)
- 내부 task: 없음

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib store::db::tests::test_list_sessions_multi_tag
cargo test --all
cd web && pnpm typecheck && pnpm build

# 라이브:
# curl "http://127.0.0.1:8080/api/sessions?tags=rust,search&page=1&page_size=5"
# 두 태그 모두 가진 세션만 반환
```

## Risks

- **단일 `tag` 후방 호환**: Obsidian 플러그인이 사용하지 않지만 향후 클라이언트 호환을 위해 유지. 두 필드 동시 사용 시 AND 매칭
- **콤마 포함 태그명**: 정규화 규칙으로 콤마 자체는 허용 안 됨 (alphanumeric + `-`/`_`만) → split 안전
- **AND vs OR**: 본 task는 AND. OR가 필요하면 별도 query param (Phase 3+)
- **빈 태그 trim**: split 후 trim + filter empty
- **date-fns `startOfWeek` weekStartsOn**: 한국 컨벤션 월요일 시작 (`weekStartsOn: 1`). 일요일 시작 원하면 0

## Scope boundary

수정 금지:
- `crates/secall-core/src/jobs/`, `web/src/components/{SearchBar,Session{ListItem,List,Detail*},TagEditor,Favorite*,Date*,Markdown*,Job*,Graph*,Command*}.tsx`
- `web/src/routes/{SessionDetail,Daily,Wiki,Commands}Route.tsx`
- `web/src/lib/{store,allTags,tagColor,utils,queryClient}.ts`
- `.github/`, `README*`
