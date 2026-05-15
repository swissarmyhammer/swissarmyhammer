import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/**
 * Progress display — renders a compact progress bar from computed
 * `{ total, completed, percent }`. Returns null when total is 0 (full
 * mode) or wraps a muted dash (compact mode) so the row honors the
 * fixed-height virtualizer contract.
 */
export function ProgressDisplay({ value, mode }: DisplayProps) {
  const obj =
    value != null && typeof value === "object"
      ? (value as Record<string, unknown>)
      : null;
  const total = obj && typeof obj.total === "number" ? obj.total : 0;
  const percent = obj && typeof obj.percent === "number" ? obj.percent : 0;

  if (mode === "compact") {
    if (total === 0) {
      return (
        <CompactCellWrapper>
          <span className="text-muted-foreground/50">-</span>
        </CompactCellWrapper>
      );
    }
    return (
      <CompactCellWrapper>
        <div className="flex items-center gap-1.5 w-full">
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
      </CompactCellWrapper>
    );
  }

  if (total === 0 || obj == null) return null;
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
