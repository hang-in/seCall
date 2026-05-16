import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { ModelsResponse } from "@/lib/types";

/**
 * P65 — backend 별 모델 목록 dynamic discovery.
 *
 * `/api/models?backend=<name>` 결과를 캐시한다. 서버 자체도 TTL 3600s 캐시 →
 * 클라이언트 staleTime 은 그보다 짧게 (10분) 잡아 UI 변경이 빠르게 반영되도록.
 * `enabled=false` 인 backend (예: 빈 문자열) 는 호출하지 않는다.
 */
export function useModels(backend: string | null | undefined, opts?: { enabled?: boolean }) {
  return useQuery<ModelsResponse>({
    queryKey: ["models", backend ?? ""],
    queryFn: () => api.listModels(backend as string),
    enabled: !!backend && (opts?.enabled ?? true),
    staleTime: 10 * 60 * 1000,
    // 백그라운드 실패해도 fallback 응답이 오기 때문에 retry 1회면 충분.
    retry: 1,
  });
}

/** 강제 새로고침 (server cache 도 무효화). */
export function useModelsRefresh() {
  const qc = useQueryClient();
  return async (backend: string) => {
    const fresh = await api.listModels(backend, true);
    qc.setQueryData<ModelsResponse>(["models", backend], fresh);
    return fresh;
  };
}
