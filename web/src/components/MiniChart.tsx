/**
 * P34 Task 07 — 세션 메타 mini-chart.
 *
 * SVG-free, recharts 미사용. 단순 div + width % 누적 막대.
 * - `RoleStackedBar`: turn role 분포 (user / assistant / system)
 * - `ToolUseList`: tool 사용 빈도 top 5 horizontal bar list
 *
 * 페이로드 + 번들 영향 < 1KB. 헤더 mini-chart 용도.
 */

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
        <div
          style={{ width: `${u}%` }}
          className="bg-blue-500/70"
          title={`user ${user}`}
        />
        <div
          style={{ width: `${a}%` }}
          className="bg-violet-500/70"
          title={`assistant ${assistant}`}
        />
        <div
          style={{ width: `${s}%` }}
          className="bg-slate-500/70"
          title={`system ${system}`}
        />
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
  const max = Math.max(...tools.map((t) => t.count));
  return (
    <div className="space-y-0.5 text-xs">
      {tools.slice(0, 5).map((t) => (
        <div key={t.name} className="flex items-center gap-2">
          <span className="w-20 truncate font-mono opacity-70">{t.name}</span>
          <div className="flex-1 h-1 rounded-full bg-muted overflow-hidden">
            <div
              style={{ width: `${(t.count / max) * 100}%` }}
              className="h-full bg-emerald-500/60"
            />
          </div>
          <span className="w-6 text-right tabular-nums opacity-70">
            {t.count}
          </span>
        </div>
      ))}
    </div>
  );
}
