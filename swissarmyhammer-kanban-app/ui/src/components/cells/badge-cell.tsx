import type { FieldDef, SelectOption } from "@/types/kanban";

/**
 * Single-badge cell for select fields.
 *
 * Looks up the matching SelectOption to resolve label and color.
 * Falls back to the raw string value when no option metadata is found.
 */
export function BadgeCell({ value, field }: { value: unknown; field: FieldDef }) {
  const text = typeof value === "string" ? value : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;

  // Try to find the option for color
  const options = (field.type as Record<string, unknown>).options as SelectOption[] | undefined;
  const option = options?.find((o) => o.value === text);
  const color = option?.color;

  return (
    <span
      className="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium bg-muted text-muted-foreground"
      style={color ? { backgroundColor: `#${color}20`, color: `#${color}` } : undefined}
    >
      {option?.label ?? text}
    </span>
  );
}
