---
type: task
status: draft
updated_at: 2026-05-02
plan_slug: p34-secall-web-phase-2-ux
task_id: 07
parallel_group: A
depends_on: []
---

# Task 07 — 세션 메타 mini-chart (turn role 분포 + tool 사용 빈도)

## Changed files

수정:
- `crates/secall-core/src/mcp/server.rs` — `do_get` 응답에 `turn_role_counts: { user, assistant, system }` + `tool_use_counts: { name → count }` 추가
- `crates/secall-core/src/store/session_repo.rs` — `get_session_stats(session_id)` 신규 — turns 테이블에서 role/tool 집계
- `web/src/lib/types.ts` — `SessionDetail`에 `turn_role_counts`, `tool_use_counts` 추가 (옵셔널)
- `web/src/components/SessionHeader.tsx` — 헤더 하단에 mini-chart 마운트

신규:
- `web/src/components/MiniChart.tsx` — 단순 SVG 가로 누적 막대 (role 분포) + horizontal bar list (tool 빈도 top 5)

## Change description

### 1. `Database::get_session_stats`

```rust
pub struct SessionStats {
    pub user_turns: i64,
    pub assistant_turns: i64,
    pub system_turns: i64,
    /// 상위 빈도 tool name → count
    pub tool_counts: Vec<(String, i64)>,
}

impl Database {
    pub fn get_session_stats(&self, session_id: &str) -> Result<SessionStats> {
        // role 카운트
        let mut stmt = self.conn().prepare(
            "SELECT role, COUNT(*) FROM turns WHERE session_id = ?1 GROUP BY role",
        )?;
        let mut user = 0i64;
        let mut assistant = 0i64;
        let mut system = 0i64;
        let rows = stmt.query_map(rusqlite::params![session_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?;
        for row in rows.filter_map(|r| r.ok()) {
            match row.0.as_str() {
                "user" => user = row.1,
                "assistant" => assistant = row.1,
                "system" => system = row.1,
                _ => {}
            }
        }

        // tool 카운트 — turns.tool_names는 JSON 배열
        let mut stmt2 = self.conn().prepare(
            "SELECT tool_names FROM turns WHERE session_id = ?1 AND has_tool = 1",
        )?;
        let mut tool_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let rows2 = stmt2.query_map(rusqlite::params![session_id], |r| r.get::<_, Option<String>>(0))?;
        for json_opt in rows2.filter_map(|r| r.ok()).flatten() {
            if let Ok(names) = serde_json::from_str::<Vec<String>>(&json_opt) {
                for name in names {
                    *tool_map.entry(name).or_insert(0) += 1;
                }
            }
        }
        let mut tool_counts: Vec<(String, i64)> = tool_map.into_iter().collect();
        tool_counts.sort_by(|a, b| b.1.cmp(&a.1));
        tool_counts.truncate(8);  // 상위 8개만

        Ok(SessionStats { user_turns: user, assistant_turns: assistant, system_turns: system, tool_counts })
    }
}
```

### 2. `do_get` 응답 보강

```rust
if let Ok(stats) = db.get_session_stats(&session_id) {
    json_val["turn_role_counts"] = serde_json::json!({
        "user": stats.user_turns,
        "assistant": stats.assistant_turns,
        "system": stats.system_turns,
    });
    json_val["tool_use_counts"] = serde_json::Value::Array(
        stats.tool_counts.into_iter().map(|(name, count)| serde_json::json!({"name": name, "count": count})).collect()
    );
}
```

### 3. `SessionDetail` 타입 확장

```ts
export interface SessionDetail {
  // ... 기존
  turn_role_counts?: { user: number; assistant: number; system: number };
  tool_use_counts?: Array<{ name: string; count: number }>;
}
```

### 4. `MiniChart.tsx`

```tsx
import { useMemo } from "react";

interface RoleProps {
  user: number;
  assistant: number;
  system: number;
}

export function RoleStackedBar({ user, assistant, system }: RoleProps) {
  const total = user + assistant + system;
  if (total === 0) return null;
  const u = (user / total) * 100;
  const a = (assistant / total) * 100;
  const s = (system / total) * 100;
  return (
    <div className="flex items-center gap-2 text-xs">
      <div className="flex-1 h-1.5 rounded-full overflow-hidden bg-muted flex">
        <div style={{ width: `${u}%` }} className="bg-blue-500/70" title={`user ${user}`} />
        <div style={{ width: `${a}%` }} className="bg-violet-500/70" title={`assistant ${assistant}`} />
        <div style={{ width: `${s}%` }} className="bg-slate-500/70" title={`system ${system}`} />
      </div>
      <span className="tabular-nums text-muted-foreground shrink-0">
        {user}u · {assistant}a{system > 0 ? ` · ${system}s` : ""}
      </span>
    </div>
  );
}

interface ToolProps {
  tools: Array<{ name: string; count: number }>;
}

export function ToolUseList({ tools }: ToolProps) {
  if (!tools.length) return null;
  const max = Math.max(...tools.map(t => t.count));
  return (
    <div className="space-y-0.5 text-xs">
      {tools.slice(0, 5).map(t => (
        <div key={t.name} className="flex items-center gap-2">
          <span className="w-20 truncate font-mono opacity-70">{t.name}</span>
          <div className="flex-1 h-1 rounded-full bg-muted overflow-hidden">
            <div style={{ width: `${(t.count / max) * 100}%` }} className="h-full bg-emerald-500/60" />
          </div>
          <span className="w-6 text-right tabular-nums opacity-70">{t.count}</span>
        </div>
      ))}
    </div>
  );
}
```

> 의존성 회피 — recharts 미사용. 단순 div + width % 로 SVG-free 차트. 약 1KB 추가만.

### 5. SessionHeader 통합

기존 SessionHeader 메타 라인 다음에 mini-chart 추가:
```tsx
{detail.turn_role_counts && (
  <RoleStackedBar {...detail.turn_role_counts} />
)}
{detail.tool_use_counts && detail.tool_use_counts.length > 0 && (
  <details className="text-xs">
    <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
      Tool 사용 ({detail.tool_use_counts.length})
    </summary>
    <div className="mt-1.5">
      <ToolUseList tools={detail.tool_use_counts} />
    </div>
  </details>
)}
```

기본은 접혀있고 `<details>` 클릭 시 펼침.

### 6. 단위 테스트

`db.rs` tests:
- `test_get_session_stats_role_distribution` — turn 5개 (user 2, assistant 3) → 정확한 카운트
- `test_get_session_stats_tool_counts` — tool_names JSON 배열 파싱 + 빈도 집계
- `test_get_session_stats_no_turns_returns_zeros`

## Dependencies

- 외부 npm: 없음 (recharts 미사용)
- 내부 task: 없음

## Verification

```bash
cargo check --all-targets
cargo clippy --all-targets --all-features
cargo fmt --all -- --check
cargo test -p secall-core --lib store::db::tests::test_get_session_stats
cargo test --all
cd web && pnpm typecheck && pnpm build

# 수동:
# /sessions/<id>에서 헤더 아래에 가로 누적 막대 (user/assistant/system 비율) + Tool 사용 details 보임
```

## Risks

- **턴 수 많은 세션 (수천 turns)**: get_session_stats가 모든 turn 스캔. 큰 세션에서 느릴 수 있음 (대부분 < 100 turns라 무시)
- **tool_names JSON 파싱 실패**: 잘못된 JSON이면 스킵. 정상적으로 영향 없음
- **응답 페이로드 증가**: `tool_use_counts`가 8개 항목 + role count → 약 200 bytes 추가. 무시
- **Obsidian 호환**: 추가 필드라 기존 클라이언트 무시
- **`<details>` accessibility**: 접근성 양호 (네이티브 disclosure widget)

## Scope boundary

수정 금지:
- `crates/secall-core/src/jobs/`, `web/`, `.github/`, `README*` 외 영역
- `crates/secall-core/src/store/{schema,db}.rs`의 마이그레이션 (Task 01만 수정)
- `crates/secall-core/src/mcp/rest.rs`의 라우트 (본 task는 server.rs do_get만)
- `web/src/components/{SearchBar,SessionFilters,SessionList*,TagEditor,Favorite*,Date*,Markdown*,Job*,Graph*,Command*,Hotkey*,RelatedSessions}.tsx`
- `web/src/routes/`
- `web/src/hooks/`, `web/src/lib/{api,store,allTags,tagColor,utils,queryClient,graphStyle,graphStartNode,highlight,hotkeyStore}.ts`
