import { useEntityStore } from "@/lib/entity-store-context";
import { useSchema, type MentionableType } from "@/lib/schema-context";
import { getStr } from "@/types/kanban";
import type { Entity, FieldDef, SelectOption } from "@/types/kanban";
import type { DisplayProps } from "./text-display";

/** Resolved label, tint color, and tooltip for a badge. */
interface BadgeResolution {
  label?: string;
  color?: string;
  /**
   * Optional tooltip text shown on hover. Populated when the target entity
   * type declares a `slugField` and the badge label is the raw id; the
   * tooltip then carries the display-field value (e.g. the project name)
   * so the user can still see the human-readable name. See the
   * `mention_slug_field` unification card for the motivation.
   */
  tooltip?: string;
}

/**
 * Resolve a reference field's value to a badge label, color, and optional tooltip.
 *
 * Looks up the target entity by `id` in the entity store, then chooses
 * the label based on whether the target entity type declares a `slugField`
 * in its schema:
 *
 * - **With `slugField` (e.g. project with `slugField: "id"`)** — the label
 *   is sourced from the slug field (typically the raw id) and the display
 *   field value is returned as `tooltip` so hovering shows the human
 *   readable name.
 * - **Without `slugField`** — preserves the legacy behavior: label comes
 *   from `mention_display_field` and no tooltip is set.
 *
 * Returns an empty object when no target entity is found, letting the
 * caller fall back to the raw value.
 */
function resolveReferenceBadge(
  value: string,
  targetEntityType: string,
  entities: Entity[],
  mentionableTypes: MentionableType[],
): BadgeResolution {
  const target = entities.find((e) => e.id === value);
  if (!target) return {};
  const mentionable = mentionableTypes.find(
    (mt) => mt.entityType === targetEntityType,
  );
  const displayField = mentionable?.displayField;
  const slugField = mentionable?.slugField;
  const color = getStr(target, "color");

  if (slugField) {
    // Slug-field path: label is the raw slug value (typically the id),
    // tooltip carries the display-field value for the hover affordance.
    const label =
      slugField === "id" ? target.id : getStr(target, slugField);
    const tooltip = displayField ? getStr(target, displayField) : "";
    return {
      label: label || undefined,
      color: color || undefined,
      tooltip: tooltip || undefined,
    };
  }

  const label = displayField ? getStr(target, displayField) : "";
  return {
    label: label || undefined,
    color: color || undefined,
  };
}

/**
 * Resolve a select field's value to a badge label and color from the
 * field's `options` array. Returns an empty object when the value doesn't
 * match any option.
 */
function resolveSelectBadge(value: string, field: FieldDef): BadgeResolution {
  const options = (field.type as Record<string, unknown>).options as
    | SelectOption[]
    | undefined;
  const option = options?.find((o) => o.value === value);
  return { label: option?.label, color: option?.color };
}

/**
 * Single badge display for scalar fields.
 *
 * Resolves the badge label and tint color from one of two field-type
 * shapes:
 *
 * 1. **Select** — `field.type.options` is a `SelectOption[]`. The badge
 *    looks up the option whose `value` matches `value` and uses its
 *    `label` and `color`.
 *
 * 2. **Reference** — `field.type.entity` names a target entity type
 *    (e.g. `"project"`). The badge looks up the entity whose `id`
 *    matches `value` in the entity store, reads the configured
 *    `mention_display_field` from the schema for the label, and reads
 *    the entity's `color` field for the tint.
 *
 * If neither lookup matches (e.g. a stale ID or an unknown option), the
 * raw value is rendered as a plain badge with no tint.
 */
export function BadgeDisplay({ value, field }: DisplayProps) {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const text = typeof value === "string" ? value : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;

  const targetEntityType = field.type.entity as string | undefined;
  const resolution: BadgeResolution = targetEntityType
    ? resolveReferenceBadge(
        text,
        targetEntityType,
        getEntities(targetEntityType),
        mentionableTypes,
      )
    : resolveSelectBadge(text, field);

  const label = resolution.label ?? text;
  const color = resolution.color;
  const tooltip = resolution.tooltip;

  // Use the native `title` attribute for hover text rather than Radix
  // Tooltip. Reference-field badges render inside task cards, so on boards
  // with thousands of tasks wrapping each badge in a Radix Tooltip (which
  // installs its own context, useEffect for delay management, and portal)
  // adds thousands of hook invocations per render and can stall the UI
  // thread during filter refetches. The native `title` is zero-cost.
  // `${color}20` appends hex alpha `20` (≈12.5% opacity) to the 6-char
  // foreground color so the background is a faded tint of the same hue —
  // identical treatment to MentionPillInner and the select-options path.
  return (
    <span
      className="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-muted text-muted-foreground"
      style={
        color
          ? { backgroundColor: `#${color}20`, color: `#${color}` }
          : undefined
      }
      // Also expose the tooltip text on a data attribute so tests can
      // inspect it without depending on native title rendering.
      data-tooltip-text={tooltip}
      title={tooltip}
    >
      {label}
    </span>
  );
}
