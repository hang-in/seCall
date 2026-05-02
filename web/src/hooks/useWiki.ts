import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { WikiPage } from "@/lib/types";

/**
 * `/api/wiki` 응답 타입.
 *
 * 백엔드 `do_wiki_search()` (crates/secall-core/src/mcp/server.rs:226)가 반환하는 구조:
 * - 본문 전체는 노출되지 않고 `preview` (앞 500자)만 포함된다.
 * - `path`는 vault 루트 기준 상대 경로 (예: `wiki/projects/seCall.md`).
 * - 본 task에서는 `preview`를 마크다운으로 렌더한다 (전체 본문 fetch 엔드포인트 없음).
 */
export interface WikiSearchResult {
  path: string;
  title: string;
  preview: string;
  created?: string;
  updated?: string;
}

export interface WikiSearchResponse {
  results: WikiSearchResult[];
  count: number;
}

export function useWikiSearch(query: string, opts?: { limit?: number; enabled?: boolean }) {
  return useQuery({
    queryKey: ["wiki", "search", query, opts?.limit ?? 5],
    queryFn: () => api.wikiSearch({ query, limit: opts?.limit ?? 5 }) as Promise<WikiSearchResponse>,
    enabled: opts?.enabled ?? !!query,
  });
}

/**
 * 단일 위키 페이지 본문 조회 — `GET /api/wiki/{project}`.
 *
 * 백엔드 `do_wiki_get()`이 `vault/wiki/projects/{safe_name}.md`를 읽어 마크다운 전체를 반환한다.
 * `project`가 undefined면 호출 자체를 비활성화하므로 라우트 진입 직후의 깜빡임을 방지한다.
 */
export function useWikiPage(project: string | undefined) {
  return useQuery<WikiPage>({
    queryKey: ["wiki", "page", project],
    queryFn: () => api.getWikiPage(project!),
    enabled: !!project,
    retry: false, // 404는 재시도 의미 없음 — 사용자가 다른 프로젝트 누르면 새 쿼리
  });
}
