import { icons } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { FieldDef } from "@/types/kanban";

/**
 * Convert a kebab-case icon name (e.g. "file-text") to the PascalCase key
 * (e.g. "FileText") used to look up components in lucide-react's `icons`
 * registry. Leading dashes and empty strings are tolerated.
 */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c) => c.toUpperCase());
}

/**
 * Resolve the lucide icon component for a field's `icon` property.
 *
 * Returns `null` when the field has no icon or when the icon name does not
 * map to a known lucide component. Callers that want a visible fallback
 * (e.g. a help glyph) should apply their own `?? Fallback` at the call site.
 *
 * @param field - The field definition whose `icon` property to resolve.
 * @returns The matching LucideIcon component, or `null` if unresolved.
 */
export function fieldIcon(field: FieldDef): LucideIcon | null {
  if (!field.icon) return null;
  const key = kebabToPascal(field.icon);
  const Icon = icons[key as keyof typeof icons];
  return Icon ?? null;
}
