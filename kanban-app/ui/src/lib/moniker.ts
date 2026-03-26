/**
 * Build a moniker string from entity type and id.
 * Format: "type:id"
 */
export function moniker(type: string, id: string): string {
  return `${type}:${id}`;
}

/**
 * Build a field-level moniker: "type:id.field"
 * Extends the entity moniker to scope focus to a specific field.
 */
export function fieldMoniker(type: string, id: string, field: string): string {
  return `${type}:${id}.${field}`;
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
