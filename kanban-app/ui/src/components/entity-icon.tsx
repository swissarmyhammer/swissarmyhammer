import { icons, LayoutGrid } from "lucide-react";
import { useSchema } from "@/lib/schema-context";

/** Convert kebab-case icon name to PascalCase key for lucide-react lookup. */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c: string) => c.toUpperCase());
}

interface EntityIconProps {
  entityType: string;
  className?: string;
}

/**
 * Renders the icon for an entity type from its YAML `icon` property.
 *
 * Resolves the icon name via the schema and does a dynamic lucide lookup.
 * Falls back to LayoutGrid if the icon name is missing or unrecognised.
 */
export function EntityIcon({ entityType, className }: EntityIconProps) {
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  const iconName = schema?.entity.icon;

  if (iconName) {
    const key = kebabToPascal(iconName);
    const Icon = icons[key as keyof typeof icons];
    if (Icon) return <Icon className={className} />;
  }

  return <LayoutGrid className={className} />;
}
