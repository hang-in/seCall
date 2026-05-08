import { useState } from "react";
import { Loader2, Play } from "lucide-react";
import { useStartJob } from "@/hooks/useJob";
import { JobOptionsDialog } from "./JobOptionsDialog";
import type {
  GraphRebuildArgs,
  IngestArgs,
  JobKind,
  SyncArgs,
  WikiUpdateArgs,
} from "@/lib/types";

interface Props {
  kind: JobKind;
  label: string;
  description: string;
}

export function CommandButton({ kind, label, description }: Props) {
  const [open, setOpen] = useState(false);
  const mutation = useStartJob(kind);

  const handleSubmit = (
    args: SyncArgs | IngestArgs | WikiUpdateArgs | GraphRebuildArgs,
  ) => {
    mutation.mutate(args);
    setOpen(false);
  };

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        disabled={mutation.isPending}
        className="text-left p-ds-4 border border-hairline rounded-lg bg-[var(--surface)] hover:bg-surface-2 hover:border-border-soft transition-colors duration-fast ease-ds disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <div className="text-t-body font-medium flex items-center gap-ds-2">
          {mutation.isPending ? (
            <Loader2 className="size-4 animate-spin text-brand" />
          ) : (
            <Play className="size-4 text-text-3" />
          )}
          {label}
        </div>
        <div className="text-t-meta text-text-3 mt-ds-1">{description}</div>
      </button>
      <JobOptionsDialog
        kind={kind}
        open={open}
        onOpenChange={setOpen}
        onSubmit={handleSubmit}
      />
    </>
  );
}
