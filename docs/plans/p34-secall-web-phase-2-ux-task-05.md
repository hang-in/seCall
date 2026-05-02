---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 05
parallel_group: B
depends_on: [00]
---

# Task 05 — 관련 세션 패널 (SessionDetail 하단)

## Changed files

수정:
- `web/src/routes/SessionDetailRoute.tsx` — 본문 아래에 `<RelatedSessions sessionId={id} />` 마운트

신규:
- `web/src/components/RelatedSessions.tsx` — 관련 세션 카드 리스트 (그래프 인접 + 같은 프로젝트/태그)
- `web/src/hooks/useRelated.ts` — `useRelated(sessionId)` 훅 — `/api/graph` + `/api/sessions?...` 조합

## Change description

### 1. 데이터 소스

세 가지 추천 source:
1. **그래프 인접 세션** — `GET /api/graph?node_id=<sessionId>&depth=1` → response의 `results` 배열에서 `node_type === "session"` 필터
2. **같은 프로젝트** — `GET /api/sessions?project=<sessionDetail.project>&page_size=5` → 자기 자신 제외
3. **같은 태그** — 첫 번째 태그 (다중이면 첫 1개만 선택) → `GET /api/sessions?tag=<tag>&page_size=5`

각 source에서 5개 가져와서 dedup (session_id 기준) → 최대 10개 표시.

### 2. `useRelated`

```ts
export function useRelated(sessionId: string | undefined) {
  const detail = useSession(sessionId, false);
  const graphQ = useQuery({
    queryKey: ["related", "graph", sessionId],
    queryFn: () => api.graph({ node_id: sessionId!, depth: 1 }),
    enabled: !!sessionId,
  });
  const project = (detail.data as any)?.project as string | undefined;
  const tag = ((detail.data as any)?.tags as string[] | undefined)?.[0];

  const projectQ = useQuery({
    queryKey: ["related", "project", project],
    queryFn: () => api.listSessions({ project, page_size: 6 }),
    enabled: !!project,
  });
  const tagQ = useQuery({
    queryKey: ["related", "tag", tag],
    queryFn: () => api.listSessions({ tag, page_size: 6 }),
    enabled: !!tag,
  });

  const items = useMemo(() => {
    const seen = new Set<string>([sessionId!].filter(Boolean) as string[]);
    const out: Array<{ id: string; reason: string; title?: string; date?: string }> = [];

    // 그래프 인접 세션
    const graphResults = (graphQ.data as any)?.results as Array<any> | undefined;
    graphResults?.filter(r => r.node_type === "session" && r.node_id !== sessionId)
      .slice(0, 5)
      .forEach(r => {
        if (!seen.has(r.node_id)) {
          seen.add(r.node_id);
          out.push({ id: r.node_id, reason: r.relation || "graph" });
        }
      });

    // 같은 프로젝트
    projectQ.data?.items.filter(s => s.id !== sessionId).slice(0, 5).forEach(s => {
      if (!seen.has(s.id)) {
        seen.add(s.id);
        out.push({ id: s.id, reason: `project:${s.project ?? ""}`, title: s.summary ?? undefined, date: s.date });
      }
    });

    // 같은 태그
    tagQ.data?.items.filter(s => s.id !== sessionId).slice(0, 5).forEach(s => {
      if (!seen.has(s.id)) {
        seen.add(s.id);
        out.push({ id: s.id, reason: `tag:${tag}`, title: s.summary ?? undefined, date: s.date });
      }
    });

    return out.slice(0, 10);
  }, [sessionId, graphQ.data, projectQ.data, tagQ.data, tag]);

  return { items, isLoading: graphQ.isLoading || projectQ.isLoading || tagQ.isLoading };
}
```

### 3. `RelatedSessions` 컴포넌트

```tsx
export function RelatedSessions({ sessionId }: { sessionId: string }) {
  const { items, isLoading } = useRelated(sessionId);
  const navigate = useNavigate();

  if (isLoading) return <div className="text-xs text-muted-foreground">관련 세션 로딩...</div>;
  if (!items.length) return null;

  return (
    <section className="mt-8 border-t border-border pt-4">
      <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
        <Network className="size-4" /> 관련 세션 ({items.length})
      </h3>
      <ul className="space-y-1.5">
        {items.map((it) => (
          <li key={it.id}>
            <button
              onClick={() => navigate(`/sessions/${it.id}`)}
              className="w-full text-left p-2 rounded hover:bg-accent text-sm flex items-center justify-between gap-2"
            >
              <span className="truncate">
                {it.title ?? it.id.slice(0, 8)}
              </span>
              <span className="text-xs text-muted-foreground tabular-nums shrink-0">
                {it.reason}{it.date ? ` · ${it.date}` : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
```

### 4. SessionDetailRoute 통합

```tsx
return (
  <div className="p-6 max-w-4xl">
    <SessionHeader id={id} detail={data} />
    {body ? <MarkdownView content={body} query={query} /> : <div>...</div>}
    <RelatedSessions sessionId={id} />
  </div>
);
```

## Dependencies

- 외부 npm: 없음
- 내부 task: Task 01 완료 권장 (notes 컬럼이 SessionListItem에 있어야 향후 확장 시 일관) — 본 task는 그래프/프로젝트/태그만 사용하므로 strict deps 아님

## Verification

```bash
cd web && pnpm typecheck
cd web && pnpm build
cargo check --all-targets

# 수동:
# /sessions/<id> 진입 → 본문 아래 "관련 세션 (N)" 패널 표시
# 그래프 인접 / 같은 프로젝트 / 같은 태그 source가 reason 라벨로 구분
# 클릭 시 해당 세션으로 이동
```

## Risks

- **API 호출 3회**: 각 SessionDetail 진입마다 graph/project/tag 3번 fetch. TanStack Query 캐시로 같은 source는 재사용. 첫 진입은 약간 느림
- **그래프 미빌드 세션**: graph_query 결과가 비면 source 1번이 빈 리스트. 다른 source가 있으면 패널 정상 표시
- **태그 1개만 사용**: 다중 태그 중 첫 번째만 추천 source로. 더 정밀하면 모든 태그 OR 매칭 (Phase 3+)
- **순서 우선순위**: 그래프 → 프로젝트 → 태그 순. 사용자가 다른 우선순위 원하면 옵션화 (Phase 3+)
- **빈 패널**: 모든 source 빈 결과 → 컴포넌트 null 반환 (출력 없음)

## Scope boundary

수정 금지:
- `crates/`, `obsidian-secall/`
- `web/src/components/{SearchBar,SessionFilters,SessionList*,SessionHeader,TagEditor,Favorite*,Date*,Markdown*,Job*,Graph*,Command*}.tsx`
- `web/src/routes/{Sessions,Daily,Wiki,Commands}Route.tsx`
- `web/src/hooks/{useSessions,useDaily,useWiki,useTagMutations,useJob*,useGraph,useGlobalHotkeys,useListHotkeys}.ts`
- `web/src/lib/{api,types,store,allTags,tagColor,utils,queryClient,graphStartNode,highlight}.ts`
- `.github/`, `README*`
