import { MentionView } from "@/components/mention-view";
import type { DisplayProps } from "./text-display";

/**
 * Single badge display for scalar reference fields.
 *
 * Delegates rendering to `MentionView` in single mode — the CM6 widget
 * pipeline owns the visible pill text (clipped display name) and tint
 * (from the target entity's `color` field).
 *
 * Behavior:
 * - When `value` is a non-empty string and `field.type.entity` names a
 *   target entity type, renders `<MentionView entityType={...} id={...} />`.
 * - When `value` is empty or not a string, shows a muted hint. If the
 *   field declares a YAML `placeholder`, that string is rendered;
 *   otherwise the legacy `-` dash fallback stays intact.
 * - When `field.type.entity` is unset (defensive guard — no shipping
 *   field has this shape), renders the raw value as a plain text span.
 *
 * The legacy `options`-based select branch was removed: no shipping field
 * definition carries `field.type.options`, so the branch was dead code.
 */
export function BadgeDisplay({ value, field }: DisplayProps) {
  const text = typeof value === "string" ? value : "";
  if (!text)
    return (
      <span className="text-muted-foreground/50">
        {field.placeholder ?? "-"}
      </span>
    );

  const targetEntityType = field.type.entity as string | undefined;
  if (!targetEntityType) return <span>{text}</span>;

  return <MentionView entityType={targetEntityType} id={text} />;
}
