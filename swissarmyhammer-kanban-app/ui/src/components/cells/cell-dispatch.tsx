import type { FieldDef, Entity } from "@/types/kanban";
import {
  resolveDisplay,
  BadgeListDisplay,
  BadgeDisplay,
  ColorSwatchDisplay,
  DateDisplay,
  NumberDisplay,
  MarkdownDisplay,
  AvatarDisplay,
  TextDisplay,
} from "@/components/fields/displays";

/** Props shared by the cell dispatch and available to individual cell renderers. */
export interface CellDisplayProps {
  field: FieldDef;
  value: unknown;
  entity: Entity;
}

/**
 * Dispatch to the correct read-only cell renderer based on field display type.
 * Uses shared display components in compact mode.
 */
export function CellDispatch({ field, value, entity }: CellDisplayProps) {
  const display = resolveDisplay(field);
  const props = { field, value, entity, mode: "compact" as const };

  switch (display) {
    case "badge-list":
      return <BadgeListDisplay {...props} />;
    case "badge":
      return <BadgeDisplay {...props} />;
    case "color-swatch":
      return <ColorSwatchDisplay {...props} />;
    case "date":
      return <DateDisplay {...props} />;
    case "number":
      return <NumberDisplay {...props} />;
    case "markdown":
      return <MarkdownDisplay {...props} />;
    case "avatar":
      return <AvatarDisplay {...props} />;
    default:
      return <TextDisplay {...props} />;
  }
}
