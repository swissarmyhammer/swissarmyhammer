import type { LucideIcon } from "lucide-react";
import type { FieldDef } from "@/types/kanban";
import { iconByName } from "@/lib/icon-name";

/**
 * Resolve the lucide icon component for a field's `icon` property.
 *
 * Delegates to `iconByName`: returns `null` when the field has no icon or
 * when the icon name does not map to a known lucide component. Callers that
 * want a visible fallback (e.g. a help glyph) should apply their own
 * `?? Fallback` at the call site.
 *
 * @param field - The field definition whose `icon` property to resolve.
 * @returns The matching LucideIcon component, or `null` if unresolved.
 */
export function fieldIcon(field: FieldDef): LucideIcon | null {
  return iconByName(field.icon);
}
