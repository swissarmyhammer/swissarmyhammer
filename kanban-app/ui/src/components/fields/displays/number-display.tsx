import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/**
 * Number display — right-aligned with tabular-nums in compact mode.
 *
 * In compact mode the output is always wrapped in {@link CompactCellWrapper}
 * — including the empty-value branch (rendered as a muted dash) — so empty
 * and populated number cells render at the same fixed pixel height for
 * the `DataTable` row virtualizer's `ROW_HEIGHT` contract.
 */
export function NumberDisplay({ value, mode }: DisplayProps) {
  const hasNumber = value != null && !Number.isNaN(Number(value));
  const num = hasNumber ? Number(value) : null;

  if (mode === "compact") {
    const inner =
      num == null ? (
        <span className="text-muted-foreground/50 ml-auto">-</span>
      ) : (
        <span className="text-sm tabular-nums text-right block ml-auto">
          {num}
        </span>
      );
    return (
      <CompactCellWrapper className="justify-end">{inner}</CompactCellWrapper>
    );
  }

  if (num == null) return null;
  return <span className="text-sm tabular-nums">{num}</span>;
}
