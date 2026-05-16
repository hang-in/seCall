import { useId } from "react";
import { Loader2, RefreshCw } from "lucide-react";
import { Input } from "@/components/ui/input";
import { useModels, useModelsRefresh } from "@/hooks/useModels";

/**
 * P65 — backend 별 모델 입력 컴포넌트.
 *
 * - `useModels(backend)` 로 `/api/models` 호출 결과를 `<datalist>` 옵션으로 노출.
 * - dynamic 실패 시 서버가 fallback list 를 돌려주므로 빈 옵션 상황은 거의 없음.
 * - 사용자는 자유 입력 가능 (모델 ID 가 비공개/실험적일 수 있어 강제 선택 X).
 * - source 가 `fallback` 이면 작은 힌트 텍스트로 알림.
 */
export function ModelInput({
  backend,
  value,
  onChange,
  disabled,
  placeholder,
  ariaLabel,
}: {
  backend: string | null | undefined;
  value: string;
  onChange: (next: string) => void;
  disabled?: boolean;
  placeholder?: string;
  ariaLabel?: string;
}) {
  const id = useId();
  const listId = `models-${id}`;
  const enabled = !!backend && backend !== "disabled";
  const { data, isLoading, isError, refetch, isFetching } = useModels(backend, {
    enabled,
  });
  const refresh = useModelsRefresh();

  const models = data?.models ?? [];
  const source = data?.source;

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2">
        <Input
          aria-label={ariaLabel}
          value={value}
          list={enabled && models.length > 0 ? listId : undefined}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          placeholder={placeholder}
          autoComplete="off"
        />
        {enabled && (
          <button
            type="button"
            onClick={async () => {
              if (!backend) return;
              await refresh(backend);
              // useQuery 쪽도 동기화
              await refetch();
            }}
            disabled={disabled || isFetching}
            className="inline-flex h-9 items-center justify-center rounded-md border border-hairline px-2 text-text-3 hover:text-text disabled:opacity-50"
            aria-label="모델 목록 새로고침"
            title="모델 목록 새로고침"
          >
            {isFetching ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <RefreshCw className="size-3.5" />
            )}
          </button>
        )}
      </div>
      {enabled && models.length > 0 && (
        <datalist id={listId}>
          {models.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
      )}
      {enabled && isLoading && (
        <p className="text-t-meta text-text-3">모델 목록 로드 중…</p>
      )}
      {enabled && !isLoading && isError && (
        <p className="text-t-meta text-status-danger">
          모델 목록 조회 실패 — 직접 입력하세요.
        </p>
      )}
      {enabled && source === "fallback" && (
        <p className="text-t-meta text-text-3">
          fallback 목록 표시 중 (backend 응답 없음 — 직접 입력 가능)
        </p>
      )}
    </div>
  );
}
