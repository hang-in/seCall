import type {
  GraphRebuildArgs,
  IngestArgs,
  JobStartResponse,
  JobState,
  ModelsResponse,
  RecallResponse,
  SessionDetail,
  SessionListPage,
  SyncArgs,
  TagsResponse,
  WikiPage,
  WikiUpdateArgs,
} from "@/lib/types";

export interface ConfigBackendConfig {
  api_url?: string | null;
  model?: string | null;
  max_tokens?: number;
}

/**
 * `/api/status` 응답.
 *
 * `version` 은 P62 부터 추가 — 서버 빌드 시점의 `CARGO_PKG_VERSION` 이라 단일 SSOT.
 */
export interface StatusResponse {
  version: string;
  sessions: number;
  turns: number;
  vectors: number;
  recent_ingests: number;
}

export interface AppConfig {
  wiki: {
    default_backend: string;
    review_model?: string | null;
    backends: Record<string, ConfigBackendConfig>;
  };
  graph: {
    semantic: boolean;
    semantic_backend: string;
    ollama_url?: string | null;
    ollama_model?: string | null;
    anthropic_model?: string | null;
    cloud_host?: string | null;
    cloud_model?: string | null;
    cloud_api_key?: string | null;
  };
  log: {
    backend?: string | null;
    model?: string | null;
    api_url?: string | null;
    max_tokens?: number | null;
    cloud_host?: string | null;
    cloud_model?: string | null;
    cloud_api_key?: string | null;
  };
  embedding: {
    backend: string;
    ollama_url?: string | null;
    ollama_model?: string | null;
    openai_model?: string | null;
    openvino_device?: string | null;
    pool_size?: number | null;
    cloud_host?: string | null;
    cloud_model?: string | null;
    cloud_api_key?: string | null;
  };
  env_indicators?: Record<string, boolean>;
}

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

  /** 세션 완전 삭제 (`DELETE /api/sessions/:id`). sessions/turns/fts/vectors 제거. */
  deleteSession: (id: string) =>
    jfetch<{ session_id: string; deleted: boolean }>(
      `/api/sessions/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    ),

  status: () => jfetch<StatusResponse>("/api/status"),

  daily: (date?: string, tzOffset?: number) =>
    jfetch<unknown>("/api/daily", {
      method: "POST",
      body: JSON.stringify({ date, tz_offset: tzOffset }),
    }),

  configGet: () => jfetch<AppConfig>("/api/config"),

  configPatch: (section: "wiki" | "graph" | "log" | "embedding", body: unknown) =>
    jfetch<AppConfig>(`/api/config/${section}`, {
      method: "PATCH",
      body: JSON.stringify(body),
    }),

  graph: (q: { node_id: string; depth?: number; relation?: string }) =>
    jfetch<unknown>("/api/graph", { method: "POST", body: JSON.stringify(q) }),

  wikiSearch: (q: { query: string; limit?: number; mode?: "keyword" | "semantic" | "hybrid" }) =>
    jfetch<unknown>("/api/wiki", { method: "POST", body: JSON.stringify(q) }),

  /** 의미 있는 그래프 subset (project + topic + agent + tool + degree top sessions). */
  graphSnapshot: (sessionLimit = 80, edgeLimit = 500) =>
    jfetch<unknown>(
      `/api/graph/snapshot?session_limit=${sessionLimit}&edge_limit=${edgeLimit}`,
      { method: "GET" },
    ),

  /** vault/wiki/projects/*.md 실존 페이지 목록. (sessions DB 의 distinct project 와 별개) */
  wikiList: () => jfetch<unknown>("/api/wiki", { method: "GET" }),

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

  /**
   * P65 — backend 별 모델 dynamic discovery.
   * `force=true` 면 서버 캐시 무시.
   */
  listModels: (backend: string, force?: boolean) =>
    jfetch<ModelsResponse>(
      `/api/models?backend=${encodeURIComponent(backend)}${force ? "&force=true" : ""}`,
    ),
};
