---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p33-secall-web-phase-1-sse-job
task_id: 04
parallel_group: A
depends_on: []
---

# Task 04 — Wiki 본문 fetch 엔드포인트 + UI

## Changed files

수정:
- `crates/secall-core/src/mcp/server.rs` — `do_wiki_get(project: &str)` 신규 메서드
- `crates/secall-core/src/mcp/rest.rs` — 신규 라우트 `GET /api/wiki/{project}` + 핸들러
- `web/src/lib/api.ts` — `getWikiPage(project)` 추가
- `web/src/hooks/useWiki.ts` — `useWikiPage(project)` 훅 추가
- `web/src/routes/WikiRoute.tsx` — preview 카드 리스트 → 단일 본문 표시 (검색은 별도 모드 유지 또는 제거)
- `web/src/lib/types.ts` — `WikiPage` 타입 추가

신규: 없음

## Change description

### 1. `do_wiki_get(project)` — 마크다운 본문 반환

`crates/secall-core/src/mcp/server.rs`에 추가:
```rust
pub fn do_wiki_get(&self, project: &str) -> anyhow::Result<serde_json::Value> {
    use std::path::PathBuf;

    // vault/wiki/projects/{safe_name}.md
    let safe_name = sanitize_project_name(project);
    let path = self.vault_path.join("wiki").join("projects").join(format!("{safe_name}.md"));

    if !path.exists() {
        return Err(anyhow::anyhow!("wiki page not found for project: {project}"));
    }

    let content = std::fs::read_to_string(&path)?;
    let metadata = std::fs::metadata(&path).ok();
    let updated = metadata
        .and_then(|m| m.modified().ok())
        .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

    Ok(serde_json::json!({
        "project": project,
        "path": path.to_string_lossy(),
        "content": content,
        "updated": updated,
    }))
}

fn sanitize_project_name(s: &str) -> String {
    // wiki 생성 시 사용한 동일 정규화 — 기존 wiki 모듈에서 import 가능 시 그것 사용
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}
```

> 정확한 sanitize 로직은 `crates/secall-core/src/wiki/lint.rs` 또는 `core/wiki/`의 기존 함수 참고. 동일 규칙 사용 필수 (mismatch 시 파일 못 찾음).

### 2. REST 라우트

`crates/secall-core/src/mcp/rest.rs`:
```rust
.route("/api/wiki/{project}", get(api_wiki_get))
```

핸들러:
```rust
async fn api_wiki_get(
    State(s): State<AppState>,
    AxumPath(project): AxumPath<String>,
) -> impl IntoResponse {
    match s.server.do_wiki_get(&project) {
        Ok(json) => (StatusCode::OK, Json(json)).into_response(),
        Err(e) => {
            // 파일 없음 → 404
            if e.to_string().contains("not found") {
                (StatusCode::NOT_FOUND, Json(json!({"error": e.to_string()}))).into_response()
            } else {
                error_response(e)
            }
        }
    }
}
```

### 3. Web `lib/api.ts`

```ts
getWikiPage: (project: string) =>
  jfetch<WikiPage>(`/api/wiki/${encodeURIComponent(project)}`),
```

### 4. Web `lib/types.ts`

```ts
export interface WikiPage {
  project: string;
  path: string;
  content: string;       // markdown body
  updated: string | null;
}
```

### 5. Web `hooks/useWiki.ts`

기존 `useWikiSearch` 유지 + `useWikiPage` 추가:
```ts
export function useWikiPage(project: string | undefined) {
  return useQuery({
    queryKey: ["wiki", "page", project],
    queryFn: () => api.getWikiPage(project!),
    enabled: !!project,
  });
}
```

### 6. `WikiRoute.tsx` 갱신

기존: 좌측 프로젝트 리스트 + 우측 검색 결과 카드 (preview 500자만)
신규: 좌측 프로젝트 리스트 + 우측 마크다운 본문 (MarkdownView)

```tsx
import { useNavigate, useParams } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { MarkdownView } from "@/components/MarkdownView";
import { useWikiPage } from "@/hooks/useWiki";

export default function WikiRoute() {
  const { project } = useParams();
  const navigate = useNavigate();

  const { data: projects } = useQuery({ queryKey: ["projects"], queryFn: api.listProjects });
  const { data: wiki, isLoading, error } = useWikiPage(project);

  return (
    <div className="grid grid-cols-[260px_1fr] h-full">
      <aside className="border-r border-border overflow-auto">
        <div className="p-3 text-xs text-muted-foreground uppercase tracking-wide">Projects</div>
        <div className="divide-y divide-border">
          {projects?.projects.map((p) => (
            <button
              key={p}
              onClick={() => navigate(`/wiki/${encodeURIComponent(p)}`)}
              className={`block w-full text-left px-3 py-2 text-sm hover:bg-accent ${p === project ? "bg-accent" : ""}`}
            >
              {p}
            </button>
          ))}
        </div>
      </aside>
      <div className="overflow-auto p-6 max-w-4xl">
        {!project ? (
          <div className="text-muted-foreground text-sm">좌측에서 프로젝트를 선택하세요</div>
        ) : isLoading ? (
          <div className="text-muted-foreground">Loading…</div>
        ) : error ? (
          <div className="text-rose-400 text-sm">
            위키 페이지를 찾을 수 없습니다: {error instanceof Error ? error.message : "unknown"}
          </div>
        ) : wiki ? (
          <>
            <header className="mb-6 pb-3 border-b border-border">
              <h1 className="text-2xl font-semibold">{wiki.project}</h1>
              {wiki.updated && (
                <div className="text-xs text-muted-foreground mt-1">
                  마지막 갱신: {wiki.updated}
                </div>
              )}
            </header>
            <MarkdownView content={wiki.content} />
          </>
        ) : null}
      </div>
    </div>
  );
}
```

### 7. 단위/통합 테스트

`crates/secall-core/tests/rest_listing.rs` 또는 `tests/wiki_get.rs` 신규에:
- vault/wiki/projects/{safe_name}.md 파일 생성 후 do_wiki_get 호출 → content 반환
- 없는 프로젝트 → Err

## Dependencies

- 외부 crate 없음
- 다른 task와 독립 (parallel_group A)

## Verification

```bash
# 1. 컴파일
cargo check -p secall-core

# 2. clippy + fmt
cargo clippy --all-targets --all-features
cargo fmt --all -- --check

# 3. 신규 테스트
cargo test -p secall-core --lib mcp::server::tests::test_do_wiki_get 2>/dev/null || \
  cargo test -p secall-core --test wiki_get 2>/dev/null || \
  echo "테스트 명 task 구현 시 확정"

# 4. Web typecheck + build
cd web && pnpm typecheck && pnpm build && cd ..

# 5. 라이브 검증
./target/release/secall serve --port 18093 &
SP=$!
sleep 3
PROJ=$(curl -s http://127.0.0.1:18093/api/projects | jq -r '.projects[0]')
echo "first project: $PROJ"
curl -s "http://127.0.0.1:18093/api/wiki/$PROJ" | jq -c '{project, content_len: (.content // "" | length), updated}'
kill $SP 2>/dev/null
```

## Risks

- **`sanitize_project_name` 정합성**: wiki 생성 시 (wiki/lint.rs 등) 사용한 규칙과 정확히 일치해야 함. 다르면 GET 시 파일 못 찾음. 가능하면 기존 함수 import 또는 동일 로직 사용
- **vault path 권한**: vault_path가 없거나 읽기 권한 없으면 500. 명확한 에러 메시지 필요
- **race condition**: wiki update job과 동시에 read하면 부분적으로 작성된 파일 읽을 수 있음. write는 atomic rename 사용 권장 (별도 issue)
- **encodeURIComponent**: 프로젝트명에 `/`, `?`, `#` 등 있으면 인코딩 필수 — `web/src/lib/api.ts`에 이미 적용됨

## Scope boundary

수정 금지:
- `crates/secall-core/src/store/`, `src/jobs/` — Task 01, 02
- `crates/secall/src/commands/` — Task 03, 08
- `web/src/routes/{Sessions,SessionDetail,Daily}Route.tsx` — Task 06이 일부 손댐
- `.github/workflows/`, `README*` — Task 09
