import { useMemo } from "react";

const CHECKBOX_RE = /- \[([ xX])\]/g;

/** Compute subtask progress from markdown checkboxes. */
export function checkboxProgress(description?: string): { checked: number; total: number } | null {
  if (!description) return null;
  let total = 0;
  let checked = 0;
  for (const match of description.matchAll(CHECKBOX_RE)) {
    total++;
    if (match[1] !== " ") checked++;
  }
  return total > 0 ? { checked, total } : null;
}

interface SubtaskProgressProps {
  description?: string;
  className?: string;
}

export function SubtaskProgress({ description, className }: SubtaskProgressProps) {
  const progress = useMemo(() => checkboxProgress(description), [description]);

  if (!progress) return null;

  const pct = Math.round((progress.checked / progress.total) * 100);

  return (
    <div className={`flex items-center gap-2 ${className ?? ""}`}>
      <div
        role="progressbar"
        aria-valuenow={pct}
        aria-valuemin={0}
        aria-valuemax={100}
        className="flex-1 h-1.5 rounded-full bg-muted overflow-hidden"
      >
        <div
          className="h-full rounded-full bg-primary transition-all duration-200"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground tabular-nums shrink-0">
        {pct}%
      </span>
    </div>
  );
}
