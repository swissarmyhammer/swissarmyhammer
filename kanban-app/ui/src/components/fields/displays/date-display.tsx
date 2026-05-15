import { formatDateForDisplay } from "@/lib/format-date";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/**
 * Date display — renders a human-friendly sentence (`"yesterday"`,
 * `"3 hours ago"`, `"Apr 12, 2026"`) produced by the shared
 * {@link formatDateForDisplay} helper. Keeps `tabular-nums` so date columns
 * still align at the character-grid level.
 *
 * The raw stored value is exposed as the native `title` tooltip so hovering
 * reveals the precise ISO timestamp without cluttering the cell.
 *
 * When the value is empty, renders the field's `description` (from the YAML
 * def) as muted help text. Falls back to the classic `-` when the schema
 * doesn't declare a description.
 *
 * In compact mode the output is wrapped in {@link CompactCellWrapper} so
 * populated and empty branches share the same fixed pixel height — required
 * by the `DataTable` row virtualizer's fixed `ROW_HEIGHT`.
 */
export function DateDisplay({ field, value, mode }: DisplayProps) {
  const text = typeof value === "string" ? value : "";
  const inner = !text ? (
    <span className="text-muted-foreground/50">{field.description ?? "-"}</span>
  ) : (
    <span className="text-sm tabular-nums" title={text}>
      {formatDateForDisplay(text)}
    </span>
  );
  return mode === "compact" ? (
    <CompactCellWrapper>{inner}</CompactCellWrapper>
  ) : (
    inner
  );
}
