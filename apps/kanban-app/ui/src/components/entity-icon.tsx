import { LayoutGrid } from "lucide-react";
import { useSchema } from "@/lib/schema-context";
import { iconByName } from "@/lib/icon-name";

interface EntityIconProps {
  entityType: string;
  className?: string;
}

/**
 * Renders the icon for an entity type from its YAML `icon` property.
 *
 * Resolves the icon name via the schema and looks it up with the shared
 * `iconByName` helper. Falls back to LayoutGrid if the icon name is missing
 * or unrecognised.
 */
export function EntityIcon({ entityType, className }: EntityIconProps) {
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  const Icon = iconByName(schema?.entity.icon) ?? LayoutGrid;
  return <Icon className={className} />;
}
