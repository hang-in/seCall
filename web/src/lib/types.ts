export interface SessionListItem {
  id: string;
  agent: string;
  project: string | null;
  model: string | null;
  date: string;
  start_time: string;
  turn_count: number;
  summary: string | null;
  tags: string[];
  is_favorite: boolean;
  session_type: string;
  vault_path: string | null;
}

export interface SessionListPage {
  items: SessionListItem[];
  total: number;
  page: number;
  page_size: number;
}

/**
 * `/api/get` 응답 형태.
 * `do_get()` (crates/secall-core/src/mcp/server.rs)이 SessionMeta(bm25)를 평탄화 후
 * P32 Task 06 rework로 `id`/`tags`/`is_favorite`/`turn_count`/`start_time`/`summary` 필드를 추가.
 * `full=true`일 때 `content` 필드에 마크다운 본문 추가.
 *
 * 모든 필드는 backend가 항상 채워주지만, Obsidian 플러그인 호환을 위해
 * 추가 필드는 옵셔널로 정의한다 (오래된 백엔드와 통신 시 안전).
 */
export interface SessionDetail {
  agent: string;
  model: string | null;
  project: string | null;
  date: string;
  vault_path: string | null;
  session_type: string;
  /** P32 rework: 단일 세션 메타. 캐시 의존 제거를 위해 응답에 포함. */
  id?: string;
  start_time?: string;
  turn_count?: number;
  summary?: string | null;
  tags?: string[];
  is_favorite?: boolean;
  /** P34 Task 00: 사용자 노트 (free-form markdown). 미설정 시 null. */
  notes?: string | null;
  /** P34 Task 07: turn role 분포 mini-chart용. 백엔드가 항상 채우지만 옵셔널. */
  turn_role_counts?: { user: number; assistant: number; system: number };
  /** P34 Task 07: tool 사용 빈도 top N (내림차순). 백엔드가 항상 채우지만 옵셔널. */
  tool_use_counts?: Array<{ name: string; count: number }>;
  /** full=true 응답에만 존재. vault 파일이 있으면 그 내용, 없으면 turns 합본. */
  content?: string;
}

/**
 * `/api/wiki/{project}` 응답.
 * `do_wiki_get()` (crates/secall-core/src/mcp/server.rs)이 반환하는 구조.
 * - `path`: 절대 경로 (vault_path 포함)
 * - `content`: 마크다운 본문 전체
 * - `updated`: 파일 mtime의 RFC3339 표현 (없으면 null)
 */
export interface WikiPage {
  project: string;
  path: string;
  content: string;
  updated: string | null;
}

export type SearchMode = "keyword" | "semantic";

/**
 * `/api/recall` 결과 항목 — turn 단위.
 * 백엔드 `SearchResult` (crates/secall-core/src/search/bm25.rs):
 *   { session_id, turn_index, score, bm25_score?, vector_score?, snippet, metadata: SessionMeta }
 * SessionMeta는 평탄화하여 노출한다 (UI 매핑 단순화).
 */
export interface RecallResultItem {
  session_id: string;
  turn_index: number;
  score: number;
  bm25_score?: number | null;
  vector_score?: number | null;
  snippet: string;
  metadata: {
    agent: string;
    model: string | null;
    project: string | null;
    date: string;
    vault_path: string | null;
    session_type: string;
  };
}

/**
 * `/api/recall` 응답 — `do_recall` (crates/secall-core/src/mcp/server.rs).
 * Ollama 미설치/embedding 비활성 시 `{ results: [], count: 0 }`만 반환되며 에러는 throw 안 됨.
 */
export interface RecallResponse {
  results: RecallResultItem[];
  count: number;
  related_sessions?: unknown[];
}

// ============================================================================
// Job system (P33 — Web Phase 1)
// ============================================================================

export type JobKind = "sync" | "ingest" | "wiki_update";
export type JobStatus =
  | "started"
  | "running"
  | "completed"
  | "failed"
  | "interrupted";

/**
 * 백엔드 `JobState` (snake_case 직렬화).
 * 출처: `crates/secall-core/src/mcp/jobs.rs` (P33 Task 02).
 *
 * - `result`/`metadata`는 kind에 따라 다른 구조 (SyncOutcome / IngestOutcome / WikiOutcome).
 *   본 레이어에서는 unknown으로 보관하고 표시 컴포넌트에서 캐스팅한다.
 */
export interface JobState {
  id: string;
  kind: JobKind;
  status: JobStatus;
  started_at: string;
  completed_at: string | null;
  current_phase: string | null;
  progress: number | null;
  message: string | null;
  error: string | null;
  result: unknown | null;
  metadata: unknown | null;
}

export interface JobStartResponse {
  job_id: string;
  status: "started";
}

/**
 * SSE `progress.event` 페이로드. `type`이 discriminator.
 * `initial_state`는 재접속 시 첫 프레임으로 현재 JobState 스냅샷을 전달.
 */
export type ProgressEvent =
  | { type: "initial_state"; state: JobState }
  | { type: "phase_start"; phase: string }
  | { type: "phase_complete"; phase: string; result?: unknown }
  | { type: "message"; text: string }
  | { type: "progress"; ratio: number }
  | { type: "done"; result: unknown }
  | { type: "failed"; error: string; partial_result?: unknown };

export interface SyncArgs {
  local_only?: boolean;
  dry_run?: boolean;
  no_wiki?: boolean;
  no_semantic?: boolean;
  no_graph?: boolean;
}

export interface IngestArgs {
  path?: string;
  auto?: boolean;
  cwd?: string;
  min_turns?: number;
  force?: boolean;
  no_semantic?: boolean;
  auto_graph?: boolean;
}

export interface WikiUpdateArgs {
  model?: string;
  backend?: string;
  since?: string;
  session?: string;
  dry_run?: boolean;
  review?: boolean;
  review_model?: string;
}

// Outcome 구조 (job 완료 시 result 필드).
// 백엔드는 단계별 skip 시 null을 반환할 수 있으므로 nullable 허용.
export interface SyncOutcome {
  pulled: boolean | null;
  reindexed: number | null;
  ingested: number;
  wiki_updated: boolean | null;
  pushed: boolean | null;
  partial_failure: boolean | null;
  graph_nodes_added: number | null;
  graph_edges_added: number | null;
}

export interface IngestOutcome {
  ingested: number;
  skipped: number;
  errors: number;
  skipped_min_turns: number;
  hook_failures: number;
  new_session_ids: string[];
  graph_nodes_added: number;
  graph_edges_added: number;
}

export interface WikiOutcome {
  backend: string;
  target: string;
  pages_written: number;
}

export interface SessionFilterState {
  project?: string;
  agent?: string;
  date_from?: string;
  date_to?: string;
  /** P32 호환 — 단일 태그. */
  tag?: string;
  /** P34 Task 03: 다중 태그 AND 매칭. */
  tags?: string[];
  favorite?: boolean;
}

export interface SessionsListParams extends SessionFilterState {
  q?: string;
  page?: number;
  page_size?: number;
}
