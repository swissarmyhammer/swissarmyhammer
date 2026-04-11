import { useEntityStore } from "@/lib/entity-store-context";
import { useSchema, type MentionableType } from "@/lib/schema-context";
import { getStr } from "@/types/kanban";
import type { Entity, FieldDef, SelectOption } from "@/types/kanban";
import type { DisplayProps } from "./text-display";

/** Resolved label and tint color for a badge. Both fields are optional. */
interface BadgeResolution {
  label?: string;
  color?: string;
}

/**
 * Resolve a reference field's value to a badge label and color.
 *
 * Looks up the target entity by `id` in the entity store, reads its
 * `mention_display_field` (from the schema) for the label, and reads
 * the entity's `color` field for the tint. Returns an empty object
 * when no target entity is found, letting the caller fall back to the
 * raw value.
 */
function resolveReferenceBadge(
  value: string,
  targetEntityType: string,
  entities: Entity[],
  mentionableTypes: MentionableType[],
): BadgeResolution {
  const target = entities.find((e) => e.id === value);
  if (!target) return {};
  const displayField = mentionableTypes.find(
    (mt) => mt.entityType === targetEntityType,
  )?.displayField;
  const label = displayField ? getStr(target, displayField) : "";
  const color = getStr(target, "color");
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

  return (
    <span
      className="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-muted text-muted-foreground"
      style={
        color
          ? { backgroundColor: `#${color}20`, color: `#${color}` }
          : undefined
      }
    >
      {label}
    </span>
  );
}
