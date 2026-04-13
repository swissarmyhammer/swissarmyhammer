/**
 * Shared metadata type for mention decorations and tooltips.
 *
 * Lives in its own module to avoid circular dependencies between
 * cm-mention-decorations.ts and cm-mention-tooltip.ts.
 */

/** Pattern matching valid 3-digit or 6-digit hex color codes (no leading `#`). */
const HEX_COLOR_RE = /^[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3})?$/;

/**
 * Validate and sanitize a hex color string.
 *
 * Returns the color if it matches a 3- or 6-digit hex pattern,
 * or an empty string if the input is invalid. This prevents CSS
 * injection via malicious color values interpolated into style attributes.
 *
 * @param color - Hex color code without leading `#` (e.g. `"ff0000"`).
 * @returns The validated color string, or `""` if invalid.
 */
export function sanitizeHexColor(color: string): string {
  return HEX_COLOR_RE.test(color) ? color : "";
}

/** Metadata associated with a mention slug — used by both decorations and tooltips. */
export interface MentionMeta {
  /** Hex color code without the leading `#` (e.g. `"ff0000"`). */
  color: string;
  /** Human-readable display name before slugification (e.g. `"Fix Login Bug"`). */
  displayName: string;
  /** Optional description shown in hover tooltips. */
  description?: string;
}
