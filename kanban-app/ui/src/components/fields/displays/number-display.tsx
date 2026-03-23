import { ProgressDisplay } from "./progress-display";
import type { DisplayProps } from "./text-display";

/** Number display — right-aligned with tabular-nums in compact mode. Returns null for empty values. Delegates to ProgressDisplay for { total, completed, percent } objects. */
export function NumberDisplay(props: DisplayProps) {
  const { value, mode } = props;
  if (value == null) return null;

  // Computed progress fields return { total, completed, percent }
  if (
    typeof value === "object" &&
    "total" in (value as Record<string, unknown>)
  ) {
    return <ProgressDisplay {...props} />;
  }

  const num = typeof value === "number" ? value : Number(value);
  if (Number.isNaN(num)) return null;
  return (
    <span
      className={`text-sm tabular-nums${mode === "compact" ? " text-right block" : ""}`}
    >
      {num}
    </span>
  );
}
