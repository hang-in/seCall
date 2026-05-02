import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";

/**
 * `/api/daily` 응답 타입.
 *
 * 백엔드 `do_daily()` (crates/secall-core/src/mcp/server.rs:408)가 반환하는 구조:
 * - 마크다운 본문은 없으며, 메타와 프로젝트별 그룹핑된 세션 리스트를 반환한다.
 * - `projects`는 BTreeMap으로 직렬화된 객체이며 키는 프로젝트 이름.
 */
export interface DailyProjectSession {
  session_id: string;
  summary: string;
  turn_count: number;
  /** JSON으로 직렬화된 도구 배열 문자열 (예: `["Read","Edit"]`) — 비어있으면 `"[]"`. */
  tools_used: string;
}

export interface DailyResponse {
  date: string;
  total_sessions: number;
  filtered_sessions: number;
  topics: string[];
  projects: Record<string, DailyProjectSession[]>;
}

export function useDaily(date: string) {
  return useQuery({
    queryKey: ["daily", date],
    queryFn: () => api.daily(date) as Promise<DailyResponse>,
  });
}
