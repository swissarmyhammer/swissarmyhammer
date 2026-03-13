import type { DisplayProps } from "./text-display";

/** Number display — right-aligned with tabular-nums in compact mode. */
export function NumberDisplay({ value, mode }: DisplayProps) {
  if (value == null) return <span className="text-muted-foreground/50">-</span>;
  const num = typeof value === "number" ? value : Number(value);
  if (Number.isNaN(num)) return <span className="text-muted-foreground/50">-</span>;
  return (
    <span className={`text-sm tabular-nums${mode === "compact" ? " text-right block" : ""}`}>
      {num}
    </span>
  );
}
