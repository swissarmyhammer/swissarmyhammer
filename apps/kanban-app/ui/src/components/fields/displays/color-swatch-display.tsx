import { CompactCellWrapper } from "./compact-cell-wrapper";

/**
 * Color swatch display — circular swatch next to the hex code.
 *
 * In compact mode the output is wrapped in {@link CompactCellWrapper} so
 * the row honors the fixed-height virtualizer contract; full mode renders
 * at natural content height (used in the inspector).
 */
export function ColorSwatchDisplay({
  value,
  mode,
}: {
  value: unknown;
  mode?: "compact" | "full";
}) {
  const hex = typeof value === "string" ? value : "";
  const inner = !hex ? (
    <span className="text-muted-foreground/50">-</span>
  ) : (
    <div className="flex items-center gap-1.5">
      <span
        className="inline-block w-4 h-4 rounded-full border border-border shrink-0"
        style={{ backgroundColor: `#${hex}` }}
      />
      <span className="text-xs text-muted-foreground font-mono">#{hex}</span>
    </div>
  );
  return mode === "compact" ? (
    <CompactCellWrapper>{inner}</CompactCellWrapper>
  ) : (
    inner
  );
}
