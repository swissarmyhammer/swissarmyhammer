import type { DisplayProps } from "./text-display";

/** Number display — right-aligned with tabular-nums in compact mode. Returns null for empty values. */
export function NumberDisplay({ value, mode }: DisplayProps) {
  if (value == null) return null;

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
