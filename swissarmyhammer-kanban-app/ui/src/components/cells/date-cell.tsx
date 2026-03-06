/** Date cell — renders the raw date string with tabular-nums for alignment. */
export function DateCell({ value }: { value: unknown }) {
  const text = typeof value === "string" ? value : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  return <span className="text-sm tabular-nums">{text}</span>;
}
