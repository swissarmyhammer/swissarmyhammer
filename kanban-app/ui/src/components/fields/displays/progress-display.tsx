import type { DisplayProps } from "./text-display";

/** Progress display — renders a compact progress bar from computed { total, completed, percent }. Returns null when total is 0. */
export function ProgressDisplay({ value, mode }: DisplayProps) {
  if (value == null || typeof value !== "object") return null;
  const obj = value as Record<string, unknown>;
  const total = typeof obj.total === "number" ? obj.total : 0;
  const percent = typeof obj.percent === "number" ? obj.percent : 0;

  if (total === 0) return null;

  if (mode === "compact") {
    return (
      <div className="flex items-center gap-1.5">
        <div
          role="progressbar"
          aria-valuenow={percent}
          aria-valuemin={0}
          aria-valuemax={100}
          className="flex-1 h-1.5 rounded-full bg-muted overflow-hidden"
        >
          <div
            className="h-full rounded-full bg-primary transition-all duration-200"
            style={{ width: `${percent}%` }}
          />
        </div>
        <span className="text-[10px] text-muted-foreground tabular-nums shrink-0">
          {percent}%
        </span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <div
        role="progressbar"
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
        className="flex-1 h-1.5 rounded-full bg-muted overflow-hidden"
      >
        <div
          className="h-full rounded-full bg-primary transition-all duration-200"
          style={{ width: `${percent}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground tabular-nums shrink-0">
        {obj.completed as number}/{total}
      </span>
    </div>
  );
}
