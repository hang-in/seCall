import { Network } from "lucide-react";
import { useNavigate } from "react-router";
import { AgentDot } from "@/components/AgentDot";
import { NoteEditor } from "@/components/NoteEditor";
import { useRelated } from "@/hooks/useRelated";
import type { SessionDetail } from "@/lib/types";

/**
 * 세션 상세 화면의 우측 aside — prototype route-sessions.jsx 의 pane__side 4-card 패턴 (Stage 7).
 *
 * 카드:
 * 1. 메타 KV (Turns / Tokens / Duration / Agent · Model)
 * 2. Mini-chart (turn role 분포 + tool use top 5)
 * 3. Related sessions (그래프 + 같은 프로젝트 + 같은 태그 dedup top N)
 * 4. Notes (자동 저장)
 */
interface Props {
  sessionId: string;
  detail: SessionDetail;
}

export function SessionAside({ sessionId, detail }: Props) {
  return (
    <aside className="w-full max-w-[300px] shrink-0 space-y-ds-4 lg:sticky lg:top-ds-6 lg:self-start">
      <MetaCard detail={detail} />
      <MiniChartCard detail={detail} />
      <RelatedCard sessionId={sessionId} />
      <NotesCard sessionId={sessionId} initial={detail.notes} />
    </aside>
  );
}

function Card({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <section className="rounded-lg border border-hairline bg-[var(--surface)] p-ds-3">
      <div className="flex items-center justify-between mb-ds-2">
        <div className="eyebrow">{title}</div>
        {hint}
      </div>
      <div>{children}</div>
    </section>
  );
}

function MetaCard({ detail }: { detail: SessionDetail }) {
  const turn = detail.turn_count;
  const start = detail.start_time;
  return (
    <Card title="메타">
      <dl className="space-y-ds-1 text-t-small text-text-2">
        {typeof turn === "number" && (
          <KV label="Turns" value={<span className="font-mono">{turn}</span>} />
        )}
        {start && (
          <KV
            label="시작"
            value={<span className="font-mono tabular-nums">{start.replace("T", " ").slice(0, 16)}</span>}
          />
        )}
        <KV
          label="Agent"
          value={
            <span className="inline-flex items-center gap-ds-1">
              <AgentDot agent={detail.agent} />
              <span>{detail.agent}</span>
            </span>
          }
        />
        {detail.model && (
          <KV label="Model" value={<span className="font-mono text-t-mono">{detail.model}</span>} />
        )}
        <KV label="Type" value={<span>{detail.session_type || "-"}</span>} />
        {detail.project && (
          <KV
            label="Project"
            value={<span className="font-mono text-t-mono">{detail.project}</span>}
          />
        )}
      </dl>
    </Card>
  );
}

function KV({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-ds-2">
      <dt className="text-text-3">{label}</dt>
      <dd className="text-text-2 truncate">{value}</dd>
    </div>
  );
}

function MiniChartCard({ detail }: { detail: SessionDetail }) {
  const roles = detail.turn_role_counts;
  const tools = detail.tool_use_counts ?? [];

  if (!roles && tools.length === 0) return null;

  return (
    <Card title="분포">
      {roles && (roles.user + roles.assistant + roles.system) > 0 && (
        <div className="space-y-ds-1">
          <div className="text-t-meta text-text-3">Roles</div>
          <RoleBar
            user={roles.user}
            assistant={roles.assistant}
            system={roles.system}
          />
        </div>
      )}
      {tools.length > 0 && (
        <div className="space-y-ds-1 mt-ds-3">
          <div className="text-t-meta text-text-3">Tool 사용</div>
          <ToolBars tools={tools} />
        </div>
      )}
    </Card>
  );
}

function RoleBar({
  user,
  assistant,
  system,
}: {
  user: number;
  assistant: number;
  system: number;
}) {
  const total = user + assistant + system;
  if (total === 0) return null;
  const pct = (n: number) => (n / total) * 100;
  return (
    <div className="space-y-ds-1">
      <div className="flex h-1.5 rounded-full overflow-hidden bg-surface-3">
        <div style={{ width: `${pct(user)}%`, background: "var(--info)" }} title={`user ${user}`} />
        <div
          style={{ width: `${pct(assistant)}%`, background: "var(--accent)" }}
          title={`assistant ${assistant}`}
        />
        <div
          style={{ width: `${pct(system)}%`, background: "var(--text-4)" }}
          title={`system ${system}`}
        />
      </div>
      <div className="font-mono text-t-meta text-text-3 tabular-nums">
        {user}u · {assistant}a{system > 0 ? ` · ${system}s` : ""}
      </div>
    </div>
  );
}

function ToolBars({ tools }: { tools: Array<{ name: string; count: number }> }) {
  const top = tools.slice(0, 5);
  const max = Math.max(...top.map((t) => t.count));
  return (
    <ul className="space-y-1 text-t-meta">
      {top.map((t) => (
        <li key={t.name} className="flex items-center gap-ds-2">
          <span className="w-16 truncate font-mono text-t-mono text-text-3">{t.name}</span>
          <div className="flex-1 h-1 rounded-full bg-surface-3 overflow-hidden">
            <div
              className="h-full bg-brand"
              style={{ width: `${(t.count / max) * 100}%`, opacity: 0.7 }}
            />
          </div>
          <span className="w-6 text-right font-mono tabular-nums text-text-3">
            {t.count}
          </span>
        </li>
      ))}
    </ul>
  );
}

function RelatedCard({ sessionId }: { sessionId: string }) {
  const { items, isLoading } = useRelated(sessionId);
  const navigate = useNavigate();

  if (isLoading) {
    return (
      <Card title="Related">
        <div className="text-t-meta text-text-3 italic">불러오는 중…</div>
      </Card>
    );
  }
  if (!items.length) return null;

  return (
    <Card
      title="Related"
      hint={
        <span className="inline-flex items-center gap-1 text-t-meta text-text-3">
          <Network className="size-3" />
          {items.length}
        </span>
      }
    >
      <ul className="space-y-ds-1">
        {items.slice(0, 8).map((it) => (
          <li key={it.id}>
            <button
              type="button"
              onClick={() => navigate(`/sessions/${encodeURIComponent(it.id)}`)}
              className="w-full text-left p-ds-1 -mx-ds-1 rounded-md hover:bg-surface-2 transition-colors duration-fast ease-ds"
            >
              <div className="text-t-small text-text-2 truncate">
                {it.title ?? it.id.slice(0, 8)}
              </div>
              <div className="font-mono text-t-meta text-text-3 tabular-nums">
                {it.reason}
                {it.date ? ` · ${it.date}` : ""}
              </div>
            </button>
          </li>
        ))}
      </ul>
    </Card>
  );
}

function NotesCard({
  sessionId,
  initial,
}: {
  sessionId: string;
  initial: string | null | undefined;
}) {
  return (
    <Card
      title="Notes"
      hint={<kbd className="kbd">e</kbd>}
    >
      <NoteEditor sessionId={sessionId} initial={initial} />
    </Card>
  );
}
