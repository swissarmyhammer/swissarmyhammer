/** Numeric cell — right-aligned with tabular-nums for column alignment. */
export function NumberCell({ value }: { value: unknown }) {
  if (value == null) return <span className="text-muted-foreground/50">-</span>;
  const num = typeof value === "number" ? value : Number(value);
  if (Number.isNaN(num)) return <span className="text-muted-foreground/50">-</span>;
  return <span className="text-sm tabular-nums text-right block">{num}</span>;
}
