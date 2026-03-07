import type { DisplayProps } from "./text-display";

/** Date display — tabular-nums for column alignment. */
export function DateDisplay({ value }: DisplayProps) {
  const text = typeof value === "string" ? value : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  return <span className="text-sm tabular-nums">{text}</span>;
}
