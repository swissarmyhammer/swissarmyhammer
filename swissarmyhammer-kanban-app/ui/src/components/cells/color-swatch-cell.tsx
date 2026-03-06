/** Color cell — renders a circular swatch next to the hex code. */
export function ColorSwatchCell({ value }: { value: unknown }) {
  const hex = typeof value === "string" ? value : "";
  if (!hex) return <span className="text-muted-foreground/50">-</span>;
  return (
    <div className="flex items-center gap-1.5">
      <span
        className="inline-block w-4 h-4 rounded-full border border-border shrink-0"
        style={{ backgroundColor: `#${hex}` }}
      />
      <span className="text-xs text-muted-foreground font-mono">#{hex}</span>
    </div>
  );
}
