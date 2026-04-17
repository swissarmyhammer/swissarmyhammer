import type { FieldDisplayProps } from "../field";
import type { Entity } from "@/types/kanban";

/**
 * Display props with entity guaranteed present.
 *
 * Derived from FieldDisplayProps (the registry contract) with `entity`
 * narrowed to non-optional. Field always passes the entity, so display
 * components can rely on it without null checks.
 */
export type DisplayProps = Omit<FieldDisplayProps, "entity"> & {
  entity: Entity;
};

/**
 * Shared text rendering primitive for field displays.
 *
 * Ensures consistent sizing, color, truncation, and empty-state rendering
 * across all display components. Displays that compute a string value
 * (e.g. TextDisplay, StatusDateDisplay) delegate here instead of rolling
 * their own `<span>` with ad-hoc classes. The parent layout owns text size
 * and color in compact mode — this primitive only inherits them — which
 * keeps card cells and inspector rows visually consistent field-to-field.
 *
 * @param text - The string to render. Empty string renders a muted dash.
 * @param mode - `compact` (list/card cells) or `full` (inspector rows).
 * @param title - Optional native tooltip text applied to the rendered span.
 */
export function DisplayText({
  text,
  mode,
  title,
}: {
  text: string;
  mode: "compact" | "full";
  title?: string;
}) {
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  if (mode === "compact")
    return (
      <span className="truncate block" title={title}>
        {text}
      </span>
    );
  return (
    <span className="text-sm" title={title}>
      {text}
    </span>
  );
}

/**
 * Plain text display — stringifies the incoming value and delegates to
 * {@link DisplayText} for rendering. Truncates in compact mode, inherits
 * sizing and color from the parent layout.
 */
export function TextDisplay({ value, mode }: DisplayProps) {
  const text =
    typeof value === "string" ? value : value != null ? String(value) : "";
  return <DisplayText text={text} mode={mode} />;
}
