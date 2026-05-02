---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 02
parallel_group: A
depends_on: []
---

# Task 02 — 검색어 하이라이트

## Changed files

수정:
- `web/src/components/SessionListItem.tsx` — summary 안의 매칭 term 강조
- `web/src/components/MarkdownView.tsx` — 마크다운 본문 안의 매칭 term 강조
- `web/src/routes/SessionDetailRoute.tsx` — query를 SessionsRoute에서 받아 MarkdownView로 prop 전달
- `web/src/routes/SessionsRoute.tsx` — 우측 outlet의 자식이 `query`를 받을 수 있도록 context 또는 outlet context 사용

신규:
- `web/src/lib/highlight.ts` — query → 정규화된 토큰 + `highlightTerms(text, terms): ReactNode[]` 유틸

## Change description

### 1. `highlight.ts`

```ts
import type { ReactNode } from "react";

/**
 * 검색 쿼리를 매칭에 사용할 토큰 배열로 분리.
 * - 공백/구두점으로 split
 * - 1글자 이하 제거 (한글 음절 단위는 별도 검토 — Phase 3+)
 * - 중복 제거 + 길이 내림차순 정렬 (긴 토큰 우선 매칭)
 * - 케이스 무시
 */
export function tokenizeQuery(query: string): string[] {
  return Array.from(
    new Set(
      query
        .toLowerCase()
        .split(/[\s,.;:!?()\[\]{}<>"'/\\]+/)
        .filter((t) => t.length > 1),
    ),
  ).sort((a, b) => b.length - a.length);
}

/**
 * 텍스트에서 토큰 매칭 부분을 <mark>로 감싼 ReactNode 배열 반환.
 * 매칭은 case-insensitive substring (regex 특수문자 escape).
 */
export function highlightTerms(text: string, terms: string[]): ReactNode[] {
  if (terms.length === 0 || !text) return [text];
  const escaped = terms.map(escapeRegex).join("|");
  const re = new RegExp(`(${escaped})`, "gi");
  const parts = text.split(re);
  return parts.map((part, i) =>
    re.test(part)
      ? <mark key={i} className="bg-amber-500/30 text-amber-100 px-0.5 rounded">{part}</mark>
      : part,
  );
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
```

> 주의: `re.test`는 stateful (g flag). 매번 새로 만들거나 `RegExp(pattern, "i")`로 stateless 사용. 위 코드는 `parts.map` 안에서 `re.test(part)` 호출 시 lastIndex 문제 — 안전하게 두 번째 regex (no g) 사용으로 변경:
> ```ts
> const matchRe = new RegExp(`^(${escaped})$`, "i");
> parts.map((part, i) => matchRe.test(part) ? <mark>{part}</mark> : part);
> ```

### 2. SessionListItem 적용

```tsx
import { highlightTerms, tokenizeQuery } from "@/lib/highlight";

interface Props {
  session: SessionListItem;
  query?: string;
  selected: boolean;
  onSelect: () => void;
}

export function SessionListItem({ session, query, selected, onSelect }: Props) {
  const terms = useMemo(() => tokenizeQuery(query || ""), [query]);
  return (
    <button onClick={onSelect} className="...">
      ...
      {session.summary && <p>{highlightTerms(session.summary, terms)}</p>}
      ...
    </button>
  );
}
```

### 3. MarkdownView 적용

react-markdown은 children renderer로 직접 텍스트 가공 가능. `components.text` (커스텀 text 노드 처리) 또는 `rehype` 플러그인 — 본 task는 단순화 위해 단일 매칭만:

```tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { highlightTerms, tokenizeQuery } from "@/lib/highlight";

interface Props {
  content: string;
  query?: string;
}

export function MarkdownView({ content, query }: Props) {
  const terms = useMemo(() => tokenizeQuery(query || ""), [query]);
  if (terms.length === 0) {
    return (
      <div className="prose prose-invert prose-sm max-w-none">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{content}</ReactMarkdown>
      </div>
    );
  }
  // 매칭이 있을 때만 children renderer 적용
  const components = {
    p: ({ children }: any) => <p>{wrapChildren(children, terms)}</p>,
    li: ({ children }: any) => <li>{wrapChildren(children, terms)}</li>,
    code: ({ children }: any) => <code>{wrapChildren(children, terms)}</code>,
  };
  return (
    <div className="prose prose-invert prose-sm max-w-none">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>{content}</ReactMarkdown>
    </div>
  );
}

function wrapChildren(children: any, terms: string[]): any {
  if (typeof children === "string") return highlightTerms(children, terms);
  if (Array.isArray(children)) return children.map((c, i) => typeof c === "string" ? highlightTerms(c, terms).map((x, j) => <Fragment key={`${i}-${j}`}>{x}</Fragment>) : c);
  return children;
}
```

> 코드블록 syntax highlight와 충돌 검토 — code 안에서 highlight 적용 시 색 충돌 가능. 본 task는 적용하되 P35 codesplit 시 syntax highlighter 도입 시 재조정.

### 4. query 전달

SessionsRoute에서 query state를 outlet context로 전달:
```tsx
import { Outlet } from "react-router";

<Outlet context={{ query, mode }} />
```

SessionDetailRoute에서:
```tsx
import { useOutletContext } from "react-router";
const { query } = useOutletContext<{ query: string; mode: SearchMode }>();
return <MarkdownView content={body} query={query} />;
```

### 5. CSS / 다크 테마

`mark` 태그는 브라우저 기본 노란 배경. 다크 모드와 충돌 → `bg-amber-500/30 text-amber-100 px-0.5 rounded` 같은 Tailwind 적용.

## Dependencies

- 외부 crate: 없음
- 내부 task: 없음 (Task 02 완료 권장이지만 strict deps 아님 — query state만 공유)

## Verification

```bash
cd web && pnpm typecheck
cd web && pnpm build
# 수동:
# /sessions에서 "rust"로 검색 → 좌측 리스트 항목의 summary에 "rust" 강조
# 세션 클릭 후 우측 마크다운 본문에서도 강조 확인
# 검색어 비우면 강조 사라짐
```

## Risks

- **regex 성능**: 토큰 많고 본문 큰 경우 (수십 KB markdown) re.test/split 비용. 대부분 세션 < 10KB라 문제 없음
- **markdown AST와 highlight 충돌**: code/link/heading 안의 매칭 강조가 marker로 깨질 수 있음. components override는 `p/li/code`만. heading/link은 highlight 안 됨 — acceptable
- **한국어 매칭**: tokenizeQuery가 공백 기준 split. 한국어 어절은 가능하지만 형태소 단위 매칭은 안 됨. P35+에서 lindera 사용 검토
- **`<mark>` 접근성**: 시각만 — 스크린리더는 읽지 않음. aria-label 추가 검토 (P35)

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`
- `web/src/components/{SearchBar,SessionFilters,Session*Header,TagEditor,Favorite*,Date*,Job*,Graph*,Command*}.tsx`
- `web/src/routes/{Daily,Wiki,Commands}Route.tsx`
- `web/src/hooks/`
- `web/src/lib/{api,types,store,allTags,tagColor,utils,queryClient}.ts`
- `.github/`, `README*`
