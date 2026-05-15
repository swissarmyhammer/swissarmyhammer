/**
 * CM6 WidgetType that visually replaces mention slug text with the
 * entity's clipped display name, while keeping the underlying slug
 * text in the document unchanged.
 */

import { WidgetType } from "@codemirror/view";
import { sanitizeHexColor } from "@/lib/mention-meta";

/**
 * Truncate a display name to a maximum character length.
 *
 * If the name exceeds `maxChars`, it is cut at `maxChars` characters
 * and an ellipsis (U+2026) is appended.
 *
 * @param name - The display name to clip.
 * @param maxChars - Maximum characters before truncation (default 24).
 * @returns The original name if within the limit, or the truncated name with ellipsis.
 */
export function clipDisplayName(name: string, maxChars = 24): string {
  if (name.length <= maxChars) return name;
  return name.slice(0, maxChars) + "\u2026";
}

/**
 * CM6 widget that renders a mention as a colored pill showing the
 * entity's display name (clipped) instead of the raw slug text.
 *
 * The underlying document text is unchanged; this widget is used
 * via `Decoration.replace` to visually substitute the slug.
 */
export class MentionWidget extends WidgetType {
  constructor(
    readonly prefix: string,
    readonly slug: string,
    readonly displayName: string,
    readonly color: string,
  ) {
    super();
  }

  /**
   * Build the DOM element for this widget.
   *
   * Returns a `<span>` styled as a pill with the entity's color applied
   * via a `--mention-color` CSS custom property. Text content is
   * `${prefix}${clipDisplayName(displayName)}`.
   */
  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-mention-pill";
    span.textContent = `${this.prefix}${clipDisplayName(this.displayName)}`;
    const safeColor = sanitizeHexColor(this.color);
    if (safeColor) {
      span.setAttribute("style", `--mention-color: #${safeColor}`);
    }
    return span;
  }

  /**
   * Compare two MentionWidget instances for equality.
   *
   * CM6 uses this to avoid re-creating DOM when decorations haven't
   * changed. All four fields must match.
   */
  eq(other: MentionWidget): boolean {
    return (
      this.prefix === other.prefix &&
      this.slug === other.slug &&
      this.displayName === other.displayName &&
      this.color === other.color
    );
  }

  /**
   * Let click events propagate through so the editor can handle them
   * (e.g. placing cursor, context menus).
   */
  ignoreEvent(): boolean {
    return false;
  }
}
