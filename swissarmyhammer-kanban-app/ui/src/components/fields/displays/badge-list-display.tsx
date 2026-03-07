import { TagPill } from "@/components/tag-pill";
import { useEntityStore } from "@/lib/entity-store-context";
import type { DisplayProps } from "./text-display";

/** Badge list display — renders tag pills. Compact: "-" when empty, full: "None" italic. */
export function BadgeListDisplay({ value, entity, mode }: DisplayProps) {
  const { getEntities } = useEntityStore();
  const slugs = Array.isArray(value) ? (value as string[]) : [];

  if (slugs.length === 0) {
    return mode === "compact"
      ? <span className="text-muted-foreground/50">-</span>
      : <span className="text-sm text-muted-foreground italic">None</span>;
  }

  const tags = getEntities("tag");
  return (
    <div className="flex flex-wrap gap-1">
      {slugs.map((slug) => (
        <TagPill key={slug} slug={slug} tags={tags} taskId={entity.id} />
      ))}
    </div>
  );
}
