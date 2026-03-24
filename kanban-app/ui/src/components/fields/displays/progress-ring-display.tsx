import { RadialBarChart, RadialBar, PolarAngleAxis } from "recharts";
import type { DisplayProps } from "./text-display";

/**
 * Radial progress ring display using Recharts.
 *
 * Handles both backend data shapes:
 * - board percent_complete: `{ done: number, total: number, percent: number }`
 * - task progress: `{ total: number, completed: number, percent: number }`
 *
 * Returns null when total is 0 or value is invalid.
 */
export function ProgressRingDisplay({ value, mode }: DisplayProps) {
  if (value == null || typeof value !== "object") return null;
  const obj = value as Record<string, unknown>;
  const total = typeof obj.total === "number" ? obj.total : 0;
  const done =
    typeof obj.done === "number"
      ? obj.done
      : typeof obj.completed === "number"
        ? obj.completed
        : 0;
  const percent = typeof obj.percent === "number" ? obj.percent : 0;

  if (total === 0) return null;

  const data = [{ value: percent, fill: "var(--color-chart-2)" }];

  const ring = (
    <div
      className="relative flex items-center justify-center w-7 h-7 shrink-0"
      role="progressbar"
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <RadialBarChart
        width={28}
        height={28}
        cx={14}
        cy={14}
        innerRadius={9}
        outerRadius={13}
        barSize={4}
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
      <div className="flex items-center gap-1.5">
        {ring}
        <span className="text-xs text-muted-foreground tabular-nums">
          {percent}%
        </span>
      </div>
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
