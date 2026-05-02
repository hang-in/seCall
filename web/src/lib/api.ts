import type {
  GraphRebuildArgs,
  IngestArgs,
  JobStartResponse,
  JobState,
  RecallResponse,
  SessionDetail,
  SessionListPage,
  SyncArgs,
  TagsResponse,
  WikiPage,
  WikiUpdateArgs,
} from "@/lib/types";

const BASE = ""; // dev/prod 모두 same-origin

async function jfetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + path, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status}: ${text}`);
  }
  return res.json();
}

export const api = {
  recall: (q: {
    query: string;
    mode?: "keyword" | "semantic" | "temporal";
    limit?: number;
    project?: string;
    agent?: string;
  }) =>
    jfetch<RecallResponse>("/api/recall", {
      method: "POST",
      body: JSON.stringify(q),
    }),

  getSession: (session_id: string, full = false) =>
    jfetch<SessionDetail>("/api/get", {
      method: "POST",
      body: JSON.stringify({ session_id, full }),
    }),

  listSessions: (
    params: Partial<{
      page: number;
      page_size: number;
      project: string;
      agent: string;
      date_from: string;
      date_to: string;
      tag: string;
      /** P34 Task 03: 다중 태그 AND. 콤마 구분 query string으로 직렬화. */
      tags: string[];
      favorite: boolean;
      q: string;
    }>,
  ) => {
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

  listProjects: () => jfetch<{ projects: string[] }>("/api/projects"),
  listAgents: () => jfetch<{ agents: string[] }>("/api/agents"),

  listTags: (withCounts: boolean = true) =>
    jfetch<TagsResponse>(
      `/api/tags?with_counts=${withCounts ? "true" : "false"}`,
    ),

  setTags: (id: string, tags: string[]) =>
    jfetch<{ session_id: string; tags: string[] }>(
      `/api/sessions/${encodeURIComponent(id)}/tags`,
      { method: "PATCH", body: JSON.stringify({ tags }) },
    ),

  setFavorite: (id: string, favorite: boolean) =>
    jfetch<{ session_id: string; favorite: boolean }>(
      `/api/sessions/${encodeURIComponent(id)}/favorite`,
      { method: "PATCH", body: JSON.stringify({ favorite }) },
    ),

  setNotes: (id: string, notes: string | null) =>
    jfetch<{ session_id: string; notes: string | null }>(
      `/api/sessions/${encodeURIComponent(id)}/notes`,
      { method: "PATCH", body: JSON.stringify({ notes }) },
    ),

  status: () => jfetch<unknown>("/api/status"),

  daily: (date?: string) =>
    jfetch<unknown>("/api/daily", {
      method: "POST",
      body: JSON.stringify({ date }),
    }),

  graph: (q: { node_id: string; depth?: number; relation?: string }) =>
    jfetch<unknown>("/api/graph", { method: "POST", body: JSON.stringify(q) }),

  wikiSearch: (q: { query: string; limit?: number }) =>
    jfetch<unknown>("/api/wiki", { method: "POST", body: JSON.stringify(q) }),

  /** 단일 위키 페이지 본문. 파일 없으면 HTTP 404 → throw. */
  getWikiPage: (project: string) =>
    jfetch<WikiPage>(`/api/wiki/${encodeURIComponent(project)}`),

  // --------------------------------------------------------------------------
  // Job system (P33) — 백엔드: crates/secall-core/src/mcp/rest_jobs.rs
  // --------------------------------------------------------------------------

  startSync: (args: SyncArgs) =>
    jfetch<JobStartResponse>("/api/commands/sync", {
      method: "POST",
      body: JSON.stringify(args),
    }),

  startIngest: (args: IngestArgs) =>
    jfetch<JobStartResponse>("/api/commands/ingest", {
      method: "POST",
      body: JSON.stringify(args),
    }),

  startWikiUpdate: (args: WikiUpdateArgs) =>
    jfetch<JobStartResponse>("/api/commands/wiki-update", {
      method: "POST",
      body: JSON.stringify(args),
    }),

  startGraphRebuild: (args: GraphRebuildArgs) =>
    jfetch<JobStartResponse>("/api/commands/graph-rebuild", {
      method: "POST",
      body: JSON.stringify(args),
    }),

  getJob: (id: string) =>
    jfetch<JobState>(`/api/jobs/${encodeURIComponent(id)}`),

  listActiveJobs: (limit?: number) => {
    const qs = new URLSearchParams({ status: "active" });
    if (typeof limit === "number") qs.set("limit", String(limit));
    return jfetch<{ jobs: JobState[] }>(`/api/jobs?${qs}`);
  },

  listRecentJobs: (limit?: number) => {
    const qs = new URLSearchParams({ status: "recent" });
    if (typeof limit === "number") qs.set("limit", String(limit));
    return jfetch<{ jobs: JobState[] }>(`/api/jobs?${qs}`);
  },

  cancelJob: (id: string) =>
    jfetch<unknown>(`/api/jobs/${encodeURIComponent(id)}/cancel`, {
      method: "POST",
    }),
};
