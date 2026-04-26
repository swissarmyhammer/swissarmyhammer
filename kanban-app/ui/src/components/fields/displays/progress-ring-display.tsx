import { RadialBarChart, RadialBar, PolarAngleAxis } from "recharts";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/**
 * Compact-mode ring diameter (px). Sized to fit inside the
 * {@link CompactCellWrapper}'s 24px height contract; the inspector-row
 * variant keeps its larger 28px ring (`FULL_RING_PX`) for legibility.
 */
const COMPACT_RING_PX = 20;
/** Inspector-row ring diameter (px) — unchanged from the original layout. */
const FULL_RING_PX = 28;

/**
 * Empty-state cell for compact mode — emits the muted-dash content inside
 * a {@link CompactCellWrapper} so a column composed entirely of empty
 * progress rings still honors `data-table.tsx::ROW_HEIGHT`. Mirrors the
 * pattern used by {@link file://./progress-display.tsx ProgressDisplay}
 * and {@link file://./status-date-display.tsx StatusDateDisplay}.
 */
function EmptyProgressRingCompact() {
  return (
    <CompactCellWrapper>
      <span className="text-muted-foreground/50">-</span>
    </CompactCellWrapper>
  );
}

/**
 * Radial progress ring display using Recharts.
 *
 * Handles both backend data shapes:
 * - board percent_complete: `{ done: number, total: number, percent: number }`
 * - task progress: `{ total: number, completed: number, percent: number }`
 *
 * In compact mode, the ring shrinks to {@link COMPACT_RING_PX} and the
 * output is wrapped in {@link CompactCellWrapper} so the row honors the
 * `DataTable` virtualizer's fixed `ROW_HEIGHT`. Empty/invalid values
 * still emit the wrapper (with a muted dash) so a column of empty rings
 * never collapses below the row's reserved height.
 *
 * In full mode (inspector rows), empty/invalid values render as `null`
 * since inspector rows compute their own height and don't carry the
 * fixed-height contract.
 */
export function ProgressRingDisplay({ value, mode }: DisplayProps) {
  if (value == null || typeof value !== "object") {
    return mode === "compact" ? <EmptyProgressRingCompact /> : null;
  }
  const obj = value as Record<string, unknown>;
  const total = typeof obj.total === "number" ? obj.total : 0;
  const done =
    typeof obj.done === "number"
      ? obj.done
      : typeof obj.completed === "number"
        ? obj.completed
        : 0;
  const percent = typeof obj.percent === "number" ? obj.percent : 0;

  if (total === 0) {
    return mode === "compact" ? <EmptyProgressRingCompact /> : null;
  }

  const data = [{ value: percent, fill: "var(--color-chart-2)" }];
  const ringPx = mode === "compact" ? COMPACT_RING_PX : FULL_RING_PX;
  const ringInner = mode === "compact" ? 6 : 9;
  const ringOuter = mode === "compact" ? 9 : 13;
  const sizeClass = mode === "compact" ? "w-5 h-5" : "w-7 h-7";

  const ring = (
    <div
      className={`relative flex items-center justify-center shrink-0 ${sizeClass}`}
      role="progressbar"
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <RadialBarChart
        width={ringPx}
        height={ringPx}
        cx={ringPx / 2}
        cy={ringPx / 2}
        innerRadius={ringInner}
        outerRadius={ringOuter}
        barSize={3}
        data={data}
        startAngle={90}
        endAngle={-270}
      >
        <PolarAngleAxis
          type="number"
          domain={[0, 100]}
          angleAxisId={0}
          tick={false}
        />
        <RadialBar
          background={{ fill: "var(--color-muted)" }}
          dataKey="value"
          cornerRadius={2}
          angleAxisId={0}
        />
      </RadialBarChart>
    </div>
  );

  if (mode === "compact") {
    return (
      <CompactCellWrapper>
        <div className="flex items-center gap-1.5">
          {ring}
          <span className="text-xs text-muted-foreground tabular-nums">
            {percent}%
          </span>
        </div>
      </CompactCellWrapper>
    );
  }

  return (
    <div className="flex items-center gap-2">
      {ring}
      <span className="text-xs text-muted-foreground tabular-nums">
        {done}/{total} ({percent}%)
      </span>
    </div>
  );
}
