const PALETTE = [
  "bg-violet-500/15 text-violet-300 ring-violet-500/30",
  "bg-cyan-500/15 text-cyan-300 ring-cyan-500/30",
  "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30",
  "bg-amber-500/15 text-amber-300 ring-amber-500/30",
  "bg-rose-500/15 text-rose-300 ring-rose-500/30",
  "bg-blue-500/15 text-blue-300 ring-blue-500/30",
  "bg-fuchsia-500/15 text-fuchsia-300 ring-fuchsia-500/30",
  "bg-teal-500/15 text-teal-300 ring-teal-500/30",
];

export function tagColor(tag: string): string {
  let hash = 0;
  for (let i = 0; i < tag.length; i++) hash = (hash * 31 + tag.charCodeAt(i)) | 0;
  return PALETTE[Math.abs(hash) % PALETTE.length];
}
