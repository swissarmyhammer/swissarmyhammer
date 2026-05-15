/**
 * Build a moniker string from entity type and id.
 * Format: "type:id"
 */
export function moniker(type: string, id: string): string {
  return `${type}:${id}`;
}

/**
 * Build a field-level moniker: "field:type:id.field"
 * Uses the "field:" namespace prefix so field monikers don't masquerade
 * as entity monikers in the scope chain.
 */
export function fieldMoniker(type: string, id: string, field: string): string {
  return `field:${type}:${id}.${field}`;
}

/**
 * Parse a moniker string into { type, id, field? }.
 * The id may contain colons (split only on the first).
 * If the id portion contains a dot, the part after the last dot is the field.
 * Throws on invalid format.
 */
export function parseMoniker(m: string): {
  type: string;
  id: string;
  field?: string;
} {
  const idx = m.indexOf(":");
  if (idx === -1) throw new Error(`Invalid moniker (no colon): "${m}"`);
  const type = m.slice(0, idx);
  const rest = m.slice(idx + 1);
  if (!type) throw new Error(`Invalid moniker (empty type): "${m}"`);
  if (!rest) throw new Error(`Invalid moniker (empty id): "${m}"`);
  const dotIdx = rest.lastIndexOf(".");
  if (dotIdx > 0) {
    return { type, id: rest.slice(0, dotIdx), field: rest.slice(dotIdx + 1) };
  }
  return { type, id: rest };
}

/**
 * Build a grid-cell moniker: `"grid_cell:{row}:{colKey}"`.
 *
 * Identifies a single cell in a grid view by its zero-based row index and
 * column key (the field name). The `grid_cell:` namespace prefix keeps cell
 * monikers separate from entity monikers so the focus chain can distinguish
 * "focus is on a grid cell" from "focus is on the underlying entity".
 *
 * @param row - Zero-based row index in the grid.
 * @param colKey - Column key (typically the field name).
 */
export function gridCellMoniker(row: number, colKey: string): string {
  return `grid_cell:${row}:${colKey}`;
}

/**
 * Parse a grid-cell moniker `"grid_cell:{row}:{colKey}"` into its components.
 *
 * Returns `null` when the moniker is not a grid-cell moniker or is malformed.
 * Callers use the `null` return to short-circuit `data-cell-cursor`
 * derivation when focus is outside the grid (e.g. on `ui:navbar`, an entity
 * moniker, or `null`).
 *
 * @param m - The moniker string to parse.
 */
export function parseGridCellMoniker(
  m: string,
): { row: number; colKey: string } | null {
  // Accept either a bare segment (`grid_cell:R:K`) or a fully-qualified
  // moniker (`/window/.../grid_cell:R:K`). Under the FQM identity model
  // the focused moniker is the full path; the trailing segment after the
  // final `/` is the spatial-segment we parse against.
  const lastSlash = m.lastIndexOf("/");
  const segment = lastSlash >= 0 ? m.slice(lastSlash + 1) : m;
  if (!segment.startsWith("grid_cell:")) return null;
  const rest = segment.slice("grid_cell:".length);
  const idx = rest.indexOf(":");
  if (idx === -1) return null;
  const rowStr = rest.slice(0, idx);
  const colKey = rest.slice(idx + 1);
  if (rowStr.length === 0 || colKey.length === 0) return null;
  const row = Number(rowStr);
  if (!Number.isInteger(row) || row < 0) return null;
  return { row, colKey };
}

/**
 * Parse a field-level moniker "field:entityType:entityId.field" into its components.
 * Throws if the moniker doesn't start with "field:" or has no field portion.
 */
export function parseFieldMoniker(m: string): {
  entityType: string;
  entityId: string;
  field: string;
} {
  const parsed = parseMoniker(m);
  if (parsed.type !== "field") {
    throw new Error(`Invalid field moniker (not a field moniker): "${m}"`);
  }
  if (!parsed.field) {
    throw new Error(`Invalid field moniker (no field): "${m}"`);
  }
  // parsed.id is "entityType:entityId", split on first colon
  const colonIdx = parsed.id.indexOf(":");
  if (colonIdx === -1) {
    throw new Error(
      `Invalid field moniker (missing entity type:id separator): "${m}"`,
    );
  }
  return {
    entityType: parsed.id.slice(0, colonIdx),
    entityId: parsed.id.slice(colonIdx + 1),
    field: parsed.field,
  };
}
