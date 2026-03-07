import type { SelectOption } from "@/types/kanban";
import type { DisplayProps } from "./text-display";

/** Single badge display for select fields — resolves label and color from options. */
export function BadgeDisplay({ value, field }: DisplayProps) {
  const text = typeof value === "string" ? value : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;

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
