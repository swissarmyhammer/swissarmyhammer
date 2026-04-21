/**
 * Reserved field name used as the moniker suffix for the synthetic
 * row-selector cell in grid views.
 *
 * The row selector is a UI-only column — it has no schema field — so
 * its moniker uses this reserved name that cannot collide with any
 * user-defined field. The leading double underscore reinforces the
 * "synthetic" nature of the name at a glance.
 *
 * Both the production `DataTable` (`components/data-table.tsx`) and
 * the spatial-nav test fixture (`test/spatial-grid-fixture.tsx`)
 * import this constant so their selector monikers can never drift.
 */
export const ROW_SELECTOR_FIELD = "__rowselector";

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
 * Build a column-header moniker: "column-header:<fieldName>".
 *
 * Used by `DataTable` to register each grid column header as its own
 * spatial focus target, so `k` (up) from a body cell lands on the header
 * directly above rather than skipping past to the perspective bar.
 *
 * The dedicated `column-header:` namespace — rather than reusing
 * `fieldMoniker` with a synthetic entity id — mirrors `moniker("view", …)`
 * used by LeftNav and keeps header rects from masquerading as body-cell
 * field entries in the scope chain.
 *
 * Future extension: if multiple grids ever coexist on screen, scope this
 * to `column-header:<perspectiveId>.<fieldName>` to keep monikers unique.
 */
export function columnHeaderMoniker(fieldName: string): string {
  return moniker("column-header", fieldName);
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
