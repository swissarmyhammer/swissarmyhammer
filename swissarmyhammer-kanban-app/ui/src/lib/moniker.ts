/**
 * Build a moniker string from entity type and id.
 * Format: "type:id"
 */
export function moniker(type: string, id: string): string {
  return `${type}:${id}`;
}

/**
 * Parse a moniker string into { type, id }.
 * The id may contain colons (split only on the first).
 * Throws on invalid format.
 */
export function parseMoniker(m: string): { type: string; id: string } {
  const idx = m.indexOf(":");
  if (idx === -1) throw new Error(`Invalid moniker (no colon): "${m}"`);
  const type = m.slice(0, idx);
  const id = m.slice(idx + 1);
  if (!type) throw new Error(`Invalid moniker (empty type): "${m}"`);
  if (!id) throw new Error(`Invalid moniker (empty id): "${m}"`);
  return { type, id };
}
