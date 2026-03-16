import {
  CheckSquare,
  Columns,
  KanbanSquare,
  Tag,
  User,
  type LucideIcon,
} from "lucide-react";

/** Map entity_type string to a Lucide icon. */
const entityTypeIcons: Record<string, LucideIcon> = {
  task: CheckSquare,
  tag: Tag,
  column: Columns,
  actor: User,
  board: KanbanSquare,
};

interface EntityIconProps {
  entityType: string;
  className?: string;
}

/**
 * Renders the icon for an entity type.
 * Falls back to the entity type name when no icon is mapped.
 */
export function EntityIcon({ entityType, className }: EntityIconProps) {
  const Icon = entityTypeIcons[entityType];
  if (Icon) {
    return <Icon className={className} />;
  }
  return (
    <span className={className}>
      {entityType.charAt(0).toUpperCase() + entityType.slice(1)}
    </span>
  );
}
