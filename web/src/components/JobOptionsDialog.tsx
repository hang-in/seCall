import { useEffect } from "react";
import { useForm, type SubmitHandler } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type {
  GraphRebuildArgs,
  IngestArgs,
  JobKind,
  SyncArgs,
  WikiUpdateArgs,
} from "@/lib/types";

// ----------------------------------------------------------------------------
// Zod 스키마 (kind별)
// ----------------------------------------------------------------------------

const syncSchema = z.object({
  local_only: z.boolean().optional(),
  dry_run: z.boolean().optional(),
  no_wiki: z.boolean().optional(),
  no_semantic: z.boolean().optional(),
  no_graph: z.boolean().optional(),
});

const ingestSchema = z.object({
  cwd: z.string().optional(),
  auto: z.boolean().optional(),
  force: z.boolean().optional(),
  // 빈 문자열을 undefined로 변환 (RHF input은 항상 string 반환)
  min_turns: z
    .union([z.string(), z.number()])
    .optional()
    .transform((v) => {
      if (v === undefined || v === "" || v === null) return undefined;
      const n = typeof v === "number" ? v : Number(v);
      return Number.isFinite(n) ? n : undefined;
    }),
  no_semantic: z.boolean().optional(),
  auto_graph: z.boolean().optional(),
});

const wikiSchema = z.object({
  backend: z.string().optional(),
  since: z.string().optional(),
  session: z.string().optional(),
  dry_run: z.boolean().optional(),
  review: z.boolean().optional(),
  review_model: z.string().optional(),
});

const graphRebuildSchema = z.object({
  since: z.string().optional(),
  session: z.string().optional(),
  all: z.boolean().optional(),
  retry_failed: z.boolean().optional(),
});

type SyncFormValues = z.input<typeof syncSchema>;
type IngestFormValues = z.input<typeof ingestSchema>;
type WikiFormValues = z.input<typeof wikiSchema>;
type GraphRebuildFormValues = z.input<typeof graphRebuildSchema>;

// 빈 문자열 필드를 제거하여 백엔드 Option<String>이 None이 되도록 한다.
function stripEmpty<T extends Record<string, unknown>>(obj: T): T {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(obj)) {
    if (v === "" || v === undefined || v === null) continue;
    out[k] = v;
  }
  return out as T;
}

interface Props {
  kind: JobKind;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (
    args: SyncArgs | IngestArgs | WikiUpdateArgs | GraphRebuildArgs,
  ) => void;
}

export function JobOptionsDialog({ kind, open, onOpenChange, onSubmit }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{labelOf(kind)} 옵션</DialogTitle>
          <DialogDescription>{descOf(kind)}</DialogDescription>
        </DialogHeader>
        {kind === "sync" && (
          <SyncForm
            onCancel={() => onOpenChange(false)}
            onSubmit={(v) => onSubmit(stripEmpty(v) as SyncArgs)}
          />
        )}
        {kind === "ingest" && (
          <IngestForm
            onCancel={() => onOpenChange(false)}
            onSubmit={(v) => onSubmit(stripEmpty(v) as IngestArgs)}
          />
        )}
        {kind === "wiki_update" && (
          <WikiForm
            onCancel={() => onOpenChange(false)}
            onSubmit={(v) => onSubmit(stripEmpty(v) as WikiUpdateArgs)}
          />
        )}
        {kind === "graph_rebuild" && (
          <GraphRebuildForm
            onCancel={() => onOpenChange(false)}
            onSubmit={(v) => onSubmit(stripEmpty(v) as GraphRebuildArgs)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function labelOf(kind: JobKind): string {
  switch (kind) {
    case "sync":
      return "Sync";
    case "ingest":
      return "Ingest";
    case "wiki_update":
      return "Wiki Update";
    case "graph_rebuild":
      return "Graph Rebuild";
  }
}

function descOf(kind: JobKind): string {
  switch (kind) {
    case "sync":
      return "git pull → reindex → ingest → push";
    case "ingest":
      return "새 세션 파싱 + 인덱스";
    case "wiki_update":
      return "LLM 백엔드로 위키 페이지 갱신";
    case "graph_rebuild":
      return "이미 ingest 된 세션의 시맨틱 그래프 재구축";
  }
}

// ----------------------------------------------------------------------------
// SyncForm
// ----------------------------------------------------------------------------

function SyncForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: SubmitHandler<SyncFormValues>;
  onCancel: () => void;
}) {
  const { register, handleSubmit, reset } = useForm<SyncFormValues>({
    resolver: zodResolver(syncSchema),
    defaultValues: {
      local_only: false,
      dry_run: false,
      no_wiki: false,
      no_semantic: false,
      no_graph: false,
    },
  });
  useEffect(() => () => reset(), [reset]);

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-3">
      <CheckboxRow label="local_only" register={register("local_only")} hint="원격 git 작업 생략" />
      <CheckboxRow label="dry_run" register={register("dry_run")} hint="실행 계획만 표시 (변경 없음)" />
      <CheckboxRow label="no_wiki" register={register("no_wiki")} hint="wiki 갱신 단계 건너뛰기" />
      <CheckboxRow label="no_semantic" register={register("no_semantic")} hint="시맨틱 임베딩 생략" />
      <CheckboxRow label="no_graph" register={register("no_graph")} hint="그래프 추출 생략" />
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>취소</Button>
        <Button type="submit">시작</Button>
      </DialogFooter>
    </form>
  );
}

// ----------------------------------------------------------------------------
// IngestForm
// ----------------------------------------------------------------------------

function IngestForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: SubmitHandler<IngestFormValues>;
  onCancel: () => void;
}) {
  const { register, handleSubmit, reset } = useForm<IngestFormValues>({
    resolver: zodResolver(ingestSchema),
    defaultValues: {
      cwd: "",
      auto: true,
      force: false,
      min_turns: "",
      no_semantic: false,
      auto_graph: false,
    },
  });
  useEffect(() => () => reset(), [reset]);

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-3">
      <div className="space-y-1">
        <label className="text-sm font-medium">cwd</label>
        <Input placeholder="(빈 값이면 현재 cwd)" {...register("cwd")} />
        <p className="text-xs text-muted-foreground">파싱 대상 작업 디렉토리</p>
      </div>
      <CheckboxRow label="auto" register={register("auto")} hint="설치된 에이전트 자동 감지" />
      <CheckboxRow label="force" register={register("force")} hint="기존 인덱스 무시하고 재처리" />
      <div className="space-y-1">
        <label className="text-sm font-medium">min_turns</label>
        <Input type="number" min={0} placeholder="(기본값 사용)" {...register("min_turns")} />
        <p className="text-xs text-muted-foreground">최소 턴 수 미만 세션은 스킵</p>
      </div>
      <CheckboxRow label="no_semantic" register={register("no_semantic")} hint="시맨틱 임베딩 생략" />
      <CheckboxRow label="auto_graph" register={register("auto_graph")} hint="ingest 직후 그래프 추출" />
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>취소</Button>
        <Button type="submit">시작</Button>
      </DialogFooter>
    </form>
  );
}

// ----------------------------------------------------------------------------
// WikiForm
// ----------------------------------------------------------------------------

const BACKENDS = ["claude", "codex", "haiku", "ollama", "lmstudio"] as const;

function WikiForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: SubmitHandler<WikiFormValues>;
  onCancel: () => void;
}) {
  const { register, handleSubmit, setValue, watch, reset } =
    useForm<WikiFormValues>({
      resolver: zodResolver(wikiSchema),
      defaultValues: {
        backend: "",
        since: "",
        session: "",
        dry_run: false,
        review: false,
        review_model: "",
      },
    });
  const backend = watch("backend") ?? "";
  useEffect(() => () => reset(), [reset]);

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-3">
      <div className="space-y-1">
        <label className="text-sm font-medium">backend</label>
        <Select
          value={backend || undefined}
          onValueChange={(v) => setValue("backend", v)}
        >
          <SelectTrigger>
            <SelectValue placeholder="(기본 백엔드)" />
          </SelectTrigger>
          <SelectContent>
            {BACKENDS.map((b) => (
              <SelectItem key={b} value={b}>
                {b}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-1">
        <label className="text-sm font-medium">since</label>
        <Input type="date" {...register("since")} />
        <p className="text-xs text-muted-foreground">이 날짜 이후 세션만 위키화</p>
      </div>
      <div className="space-y-1">
        <label className="text-sm font-medium">session</label>
        <Input placeholder="(특정 session_id 한정)" {...register("session")} />
      </div>
      <CheckboxRow label="dry_run" register={register("dry_run")} hint="실제 파일 변경 없음" />
      <CheckboxRow label="review" register={register("review")} hint="lint + LLM 리뷰 단계 활성" />
      <div className="space-y-1">
        <label className="text-sm font-medium">review_model</label>
        <Input placeholder="(빈 값이면 기본값)" {...register("review_model")} />
      </div>
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>취소</Button>
        <Button type="submit">시작</Button>
      </DialogFooter>
    </form>
  );
}

// ----------------------------------------------------------------------------
// GraphRebuildForm
// ----------------------------------------------------------------------------

function GraphRebuildForm({
  onSubmit,
  onCancel,
}: {
  onSubmit: SubmitHandler<GraphRebuildFormValues>;
  onCancel: () => void;
}) {
  const { register, handleSubmit, reset } = useForm<GraphRebuildFormValues>({
    resolver: zodResolver(graphRebuildSchema),
    defaultValues: {
      since: "",
      session: "",
      all: false,
      retry_failed: false,
    },
  });
  useEffect(() => () => reset(), [reset]);

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-3">
      <p className="text-xs text-muted-foreground border border-border rounded px-2 py-1.5">
        우선순위: <span className="font-mono">session</span> &gt;{" "}
        <span className="font-mono">all</span> &gt;{" "}
        <span className="font-mono">retry_failed</span> &gt;{" "}
        <span className="font-mono">since</span> (Task 00 SQL 기준)
      </p>
      <div className="space-y-1">
        <label className="text-sm font-medium">since</label>
        <Input type="date" {...register("since")} />
        <p className="text-xs text-muted-foreground">이 날짜 이후 세션만 대상</p>
      </div>
      <div className="space-y-1">
        <label className="text-sm font-medium">session</label>
        <Input
          placeholder="full session_id (UUID)"
          {...register("session")}
        />
        <p className="text-xs text-muted-foreground">
          전체 session_id 정확히 입력 (backend 는 exact match — prefix 미지원)
        </p>
      </div>
      <CheckboxRow label="all" register={register("all")} hint="모든 ingest된 세션 대상" />
      <CheckboxRow
        label="retry_failed"
        register={register("retry_failed")}
        hint="이전 실패/skip 세션만 재시도"
      />
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>취소</Button>
        <Button type="submit">시작</Button>
      </DialogFooter>
    </form>
  );
}

// ----------------------------------------------------------------------------
// 공용 체크박스 행
// ----------------------------------------------------------------------------

function CheckboxRow({
  label,
  register,
  hint,
}: {
  label: string;
  register: ReturnType<ReturnType<typeof useForm>["register"]>;
  hint?: string;
}) {
  return (
    <label className="flex items-start gap-2 text-sm cursor-pointer">
      <input
        type="checkbox"
        className="mt-0.5 size-4 rounded border-input"
        {...register}
      />
      <div>
        <div className="font-mono text-xs">{label}</div>
        {hint && <div className="text-xs text-muted-foreground">{hint}</div>}
      </div>
    </label>
  );
}
