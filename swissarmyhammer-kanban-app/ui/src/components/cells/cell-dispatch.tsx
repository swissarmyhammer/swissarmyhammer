import type { FieldDef, Entity } from "@/types/kanban";
import { TagPill } from "@/components/tag-pill";
import { useEntityStore } from "@/lib/entity-store-context";
import { TextCell } from "./text-cell";
import { BadgeCell } from "./badge-cell";
import { DateCell } from "./date-cell";
import { ColorSwatchCell } from "./color-swatch-cell";
import { NumberCell } from "./number-cell";

/** Props shared by the cell dispatch and available to individual cell renderers. */
export interface CellDisplayProps {
  field: FieldDef;
  value: unknown;
  entity: Entity;
}

/**
 * Dispatch to the correct read-only cell renderer based on field type/display.
 *
 * This is the grid-view counterpart of FieldDispatch in entity-inspector —
 * compact, non-editable renderers optimised for table cells.
 */
export function CellDispatch({ field, value, entity }: CellDisplayProps) {
  const { getEntities } = useEntityStore();

  // Badge list (tags) — reuse TagPill for colors + context menus
  if (field.display === "badge-list") {
    const slugs = Array.isArray(value) ? (value as string[]) : [];
    if (slugs.length === 0) return <span className="text-muted-foreground/50">-</span>;
    const tags = getEntities("tag");
    return (
      <div className="flex flex-wrap gap-1">
        {slugs.map((slug) => (
          <TagPill key={slug} slug={slug} tags={tags} taskId={entity.id} />
        ))}
      </div>
    );
  }

  // Single badge (select fields)
  if (field.display === "badge" || field.type.kind === "select") {
    return <BadgeCell value={value} field={field} />;
  }

  // Color
  if (field.type.kind === "color") {
    return <ColorSwatchCell value={value} />;
  }

  // Date
  if (field.type.kind === "date") {
    return <DateCell value={value} />;
  }

  // Number
  if (field.type.kind === "number" || field.type.kind === "integer") {
    return <NumberCell value={value} />;
  }

  // Default: text (markdown, string, etc)
  return <TextCell value={value} />;
}
