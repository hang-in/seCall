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
        className="text-left p-3 border border-border rounded hover:bg-accent transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <div className="font-medium flex items-center gap-2">
          {mutation.isPending ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Play className="size-4" />
          )}
          {label}
        </div>
        <div className="text-xs text-muted-foreground mt-1">{description}</div>
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
