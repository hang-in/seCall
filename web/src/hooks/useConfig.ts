import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/api";

export function useConfig() {
  return useQuery({
    queryKey: ["config"],
    queryFn: () => api.configGet(),
    staleTime: 10_000,
  });
}

export function useConfigPatch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      section,
      body,
    }: {
      section: "wiki" | "graph" | "log" | "embedding";
      body: unknown;
    }) => api.configPatch(section, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["config"] });
      toast.success("설정 저장됨");
    },
    onError: (err) => {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("403")) {
        toast.error("config 편집 비활성. `secall serve --allow-config-edit` 로 다시 시작하세요");
      } else {
        toast.error(`설정 저장 실패: ${msg}`);
      }
    },
  });
}
