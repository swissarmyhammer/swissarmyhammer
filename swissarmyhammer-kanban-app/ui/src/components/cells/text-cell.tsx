/** Plain text cell — truncates to a single line. */
export function TextCell({ value }: { value: unknown }) {
  const text = typeof value === "string" ? value : value != null ? String(value) : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  return <span className="truncate block">{text}</span>;
}
