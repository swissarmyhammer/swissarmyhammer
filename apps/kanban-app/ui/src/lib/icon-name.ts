import { icons } from "lucide-react";
import type { LucideIcon } from "lucide-react";

/**
 * Convert a kebab-case icon name (e.g. "file-text") to the PascalCase key
 * (e.g. "FileText") used to look up components in lucide-react's `icons`
 * registry. Leading dashes and empty strings are tolerated.
 */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c: string) => c.toUpperCase());
}

/**
 * Resolve a metadata-declared icon name to its lucide component.
 *
 * The single lookup behind `fieldIcon` and `viewIcon`: kebab-case names map
 * to lucide-react's PascalCase `icons` registry keys. Returns `null` when the
 * name is absent or does not map to a known lucide component, so callers
 * apply their own documented fallback.
 *
 * @param name - The kebab-case icon name to resolve (e.g. "file-text").
 * @returns The matching LucideIcon component, or `null` if unresolved.
 */
export function iconByName(name: string | null | undefined): LucideIcon | null {
  if (!name) return null;
  const key = kebabToPascal(name);
  return icons[key as keyof typeof icons] ?? null;
}
